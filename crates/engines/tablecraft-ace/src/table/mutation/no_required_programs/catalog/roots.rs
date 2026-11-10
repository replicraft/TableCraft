//! Complete physical catalog census and uniquely owned logical-root resolution.
use super::super::refused;
use crate::{
    EditError, InspectError,
    format::{
        bytes::{u16_at, u32_at},
        page::{DATA_SIGNATURE, OWNER_OFFSET, PAGE_BYTES},
        reader::{PhysicalRow, Reader, row_count, row_range},
        row::{
            DELETED_MASK, OFFSET_MASK, RELOCATED_MASK, UNSUPPORTED_MASK, reference, reference_page,
            reference_slot, slot_offset,
        },
    },
    header::check,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Live,
    Hidden,
    EmptyDeleted,
}

struct Slot {
    reference: u32,
    kind: Kind,
    owner: Option<u32>,
}

pub(super) struct LogicalRow {
    pub page: u32,
    pub slot: u8,
    pub physical: PhysicalRow,
}

fn reserve<T>(values: &mut Vec<T>, budget: &mut ResourceBudget) -> Result<(), InspectError> {
    let relocation_bytes = values
        .len()
        .checked_mul(size_of::<T>())
        .ok_or(InspectError::StructureLimit)?;
    budget.charge(Resource::Steps, relocation_bytes as u64)?;
    budget.charge_allocation(1, size_of::<T>() as u64)?;
    values
        .try_reserve_exact(1)
        .map_err(|_| InspectError::Allocation)
}

#[derive(Clone, Copy)]
enum Census {
    Original,
    #[cfg(feature = "ace16-create")]
    Retained,
}

pub(super) fn capture<R: crate::NativeReadSource>(
    reader: &mut Reader<'_, R>,
    owned: &[u32],
    definition: u32,
) -> Result<Vec<LogicalRow>, EditError> {
    capture_with_policy(reader, owned, definition, Census::Original)
}

/// Only a complete bound retained inventory permits orphan retired catalog bytes.
#[cfg(feature = "ace16-create")]
pub(super) fn capture_retained<R: crate::NativeReadSource>(
    reader: &mut Reader<'_, R>,
    owned: &[u32],
    definition: u32,
    inventory: &crate::table::table_append::Inventory<'_>,
) -> Result<Vec<LogicalRow>, EditError> {
    if inventory.catalog_definition() != definition
        || inventory.catalog_pages() != owned
        || inventory.before_pages() != reader.header.page_count()
    {
        return Err(refused());
    }
    capture_with_policy(reader, owned, definition, Census::Retained)
}

fn capture_with_policy<R: crate::NativeReadSource>(
    reader: &mut Reader<'_, R>,
    owned: &[u32],
    definition: u32,
    census: Census,
) -> Result<Vec<LogicalRow>, EditError> {
    let mut slots = Vec::new();
    let mut logical = Vec::new();
    let mut previous = None;
    for &number in owned {
        reader.tick()?;
        if previous.is_some_and(|prior| prior >= number) {
            return Err(refused());
        }
        previous = Some(number);
        let page = reader.page(number)?;
        if page[..DATA_SIGNATURE.len()] != DATA_SIGNATURE
            || u32_at(&page, OWNER_OFFSET)? != definition
        {
            return Err(refused());
        }
        let count = row_count(&page)?;
        for slot in 0..count {
            reader.tick()?;
            reader
                .budget
                .charge(Resource::Rows, 1)
                .map_err(InspectError::from)?;
            let word = u16_at(&page, slot_offset(slot))?;
            if word & UNSUPPORTED_MASK != 0 {
                return Err(refused());
            }
            let flags = word & !OFFSET_MASK;
            if flags & !(DELETED_MASK | RELOCATED_MASK) != 0 {
                return Err(refused());
            }
            let start = usize::from(word & OFFSET_MASK);
            let end = if slot == 0 {
                PAGE_BYTES
            } else {
                usize::from(u16_at(&page, slot_offset(slot - 1))? & OFFSET_MASK)
            };
            if start < slot_offset(count) || start > end || end > PAGE_BYTES {
                return Err(refused());
            }
            let kind = if start == end {
                if flags & DELETED_MASK == 0 {
                    return Err(refused());
                }
                Kind::EmptyDeleted
            } else {
                // Retain row_range's unsupported-bit and nonempty geometry law.
                row_range(&page, slot)?;
                if flags & DELETED_MASK != 0 {
                    Kind::Hidden
                } else {
                    Kind::Live
                }
            };
            let slot = u8::try_from(slot).map_err(|_| InspectError::StructureLimit)?;
            let key = reference(number, slot);
            reserve(&mut slots, reader.budget)?;
            slots.push(Slot {
                reference: key,
                kind,
                owner: None,
            });
            if kind == Kind::Live {
                reserve(&mut logical, reader.budget)?;
                logical.push(key);
            }
        }
    }
    let levels = slots.len().checked_ilog2().unwrap_or(0) as u64 + 1;
    let mut rows = Vec::new();
    for root in logical {
        reader.tick()?;
        let page_number = reference_page(root);
        let slot = reference_slot(root);
        let page = reader.page(page_number)?;
        let mut visit = |key: u32, budget: &mut ResourceBudget| -> Result<(), InspectError> {
            budget.charge(Resource::Steps, levels)?;
            let position = slots
                .binary_search_by_key(&key, |slot| slot.reference)
                .map_err(|_| InspectError::Malformed)?;
            let entry = &mut slots[position];
            let expected = if key == root {
                Kind::Live
            } else {
                Kind::Hidden
            };
            if entry.kind != expected || entry.owner.replace(root).is_some() {
                return Err(InspectError::Malformed);
            }
            Ok(())
        };
        let physical = match census {
            #[cfg(feature = "ace16-create")]
            Census::Retained => reader.read_location_observed(
                page_number,
                slot,
                definition,
                &page,
                &mut |reference, budget| {
                    let levels = owned.len().checked_ilog2().unwrap_or(0) as u64 + 1;
                    budget.charge(Resource::Steps, levels)?;
                    if owned.binary_search(&reference_page(reference)).is_err() {
                        return Err(InspectError::Malformed);
                    }
                    visit(reference, budget)
                },
            )?,
            _ => reader.row_location_scoped(
                page_number,
                slot,
                definition,
                &page,
                owned,
                &mut visit,
            )?,
        };
        reserve(&mut rows, reader.budget)?;
        rows.push(LogicalRow {
            page: page_number,
            slot,
            physical,
        });
    }
    for slot in slots {
        check(reader.budget, reader.cancelled)?;
        if slot.kind != Kind::EmptyDeleted && slot.owner.is_none() {
            match census {
                Census::Original => return Err(refused()),
                #[cfg(feature = "ace16-create")]
                Census::Retained if slot.kind == Kind::Hidden => {
                    // The bound whole-file inventory protects these original
                    // tombstones and old hidden targets. No new logical row,
                    // writable location or permission is invented from them.
                }
                #[cfg(feature = "ace16-create")]
                Census::Retained => return Err(refused()),
            }
        }
    }
    reader.tick()?;
    Ok(rows)
}

#[cfg(test)]
mod tests;

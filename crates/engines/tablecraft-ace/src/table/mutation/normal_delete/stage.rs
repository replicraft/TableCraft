//! Source-bound logical-root slots; resolved terminal storage remains untouched.
use crate::{
    EditError, InspectError, NativeImage, RowDelete,
    format::{
        bytes::{u16_at, u32_at},
        layout::RECORD_COUNT_OFFSET,
        page::PAGE_BYTES,
        row::{
            DELETED_MASK, OFFSET_MASK, RELOCATED_MASK, reference_page, reference_slot, slot_offset,
        },
    },
    header::check,
    table::mutation::preserved_source::Source,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

struct Slot {
    offset: usize,
    original: u16,
}

pub(super) struct Staged {
    slots: Vec<Slot>,
    count_offset: usize,
    original_count: u32,
    final_count: u32,
}

pub(super) fn prepare(
    image: &NativeImage,
    source: &Source,
    deletes: &[RowDelete],
    family: super::Family,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Staged, EditError> {
    // The root service resolves all live roots and establishes exclusive final
    // ownership before any chosen logical slot can become a staging operation.
    let captured = family.roots(image, source.table.schema(), deletes, budget, cancelled)?;
    budget
        .charge_allocation(captured.selected.len() as u64, size_of::<Slot>() as u64)
        .map_err(InspectError::from)?;
    let mut slots = Vec::new();
    slots
        .try_reserve_exact(captured.selected.len())
        .map_err(|_| InspectError::Allocation)?;
    for root in captured.selected {
        check(budget, cancelled)?;
        let record = source
            .records
            .get(root.update.row())
            .ok_or(EditError::Conflict)?;
        budget
            .charge(Resource::Steps, record.data.len() as u64)
            .map_err(InspectError::from)?;
        if record.reference != root.reference
            || record.offset != root.final_record.offset
            || record.data != root.final_record.data
        {
            return Err(EditError::Preservation);
        }
        budget
            .charge(Resource::Steps, record.data.len() as u64)
            .map_err(InspectError::from)?;
        if record.data != root.update.expected_bytes() {
            return Err(EditError::Conflict);
        }
        let page = reference_page(root.reference) as usize;
        let slot = usize::from(reference_slot(root.reference));
        let offset = page
            .checked_mul(PAGE_BYTES)
            .and_then(|base| base.checked_add(slot_offset(slot)))
            .ok_or(EditError::Preservation)?;
        if root.word & !(OFFSET_MASK | RELOCATED_MASK) != 0
            || u16_at(image.as_bytes(), offset)? != root.word
        {
            return Err(EditError::Preservation);
        }
        slots.push(Slot {
            offset,
            original: root.word,
        });
    }
    let schema = source.table.schema();
    let count_offset = (schema.definition_page() as usize)
        .checked_mul(PAGE_BYTES)
        .and_then(|base| base.checked_add(RECORD_COUNT_OFFSET))
        .ok_or(EditError::Preservation)?;
    let original_count = u32_at(image.as_bytes(), count_offset)?;
    if original_count != schema.declared_rows() || slots.len() != deletes.len() {
        return Err(EditError::Preservation);
    }
    let removed = u32::try_from(deletes.len()).map_err(|_| InspectError::StructureLimit)?;
    let final_count = original_count
        .checked_sub(removed)
        .ok_or(EditError::Conflict)?;
    check(budget, cancelled)?;
    Ok(Staged {
        slots,
        count_offset,
        original_count,
        final_count,
    })
}

impl Staged {
    pub(super) fn apply(
        self,
        after: &mut [u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        for slot in self.slots {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, size_of::<u16>() as u64)
                .map_err(InspectError::from)?;
            if u16_at(after, slot.offset)? != slot.original {
                return Err(EditError::Preservation);
            }
            let tombstone = slot.original | DELETED_MASK | RELOCATED_MASK;
            after
                .get_mut(slot.offset..slot.offset + size_of::<u16>())
                .ok_or(EditError::Preservation)?
                .copy_from_slice(&tombstone.to_le_bytes());
        }
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, size_of::<u32>() as u64)
            .map_err(InspectError::from)?;
        if u32_at(after, self.count_offset)? != self.original_count {
            return Err(EditError::Preservation);
        }
        after
            .get_mut(self.count_offset..self.count_offset + size_of::<u32>())
            .ok_or(EditError::Preservation)?
            .copy_from_slice(&self.final_count.to_le_bytes());
        check(budget, cancelled)?;
        Ok(())
    }
}

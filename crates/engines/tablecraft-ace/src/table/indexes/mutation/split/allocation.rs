//! Per-operation EOF placement with separately owned bitmap metadata.
use super::{AddedPage, AllocationPolicy, Plan, maps};
use crate::{
    EditError, IndexCheck, InspectError, NativeImage, NativeIndex, WriteBlocker,
    format::{
        allocation::{self, GLOBAL_FREE_REFERENCE, MapRole, QualifiedInlineRecord, RepresentedMap},
        page::{PAGE_BYTES, USAGE_SIGNATURE},
        reader::Reader,
    },
    table::mutation::{
        append,
        map_authority::{Owner, RecordIdentity},
    },
};
use std::io::Cursor;

const INLINE_KIND: u8 = 0;
const BITMAP_PREFIX_BYTES: usize = USAGE_SIGNATURE.len();
const BITMAP_CAPACITY: usize = (PAGE_BYTES - BITMAP_PREFIX_BYTES) * u8::BITS as usize;

#[cfg(test)]
mod tests;

pub(in crate::table) struct Promotion {
    pub usage: u32,
    pub number: u32,
    pub offset: usize,
    pub before: Vec<u8>,
    pub after: Vec<u8>,
    pub bitmap: Vec<u8>,
}

pub(in crate::table) struct Rebase {
    pub usage: u32,
    pub physical: u32,
    pub base: u32,
}

pub(super) fn reserve(
    image: &NativeImage,
    index: &NativeIndex,
    plan: &mut Plan,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<u32, EditError> {
    reserve_inner(image, index, plan, reader)
}

fn reserve_inner(
    image: &NativeImage,
    index: &NativeIndex,
    plan: &mut Plan,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<u32, EditError> {
    let number = plan.append.next()?;
    plan.append.preview(1)?;
    reader.work(plan.promotions.len())?;
    if let Some(promotion) = plan
        .promotions
        .iter_mut()
        .find(|p| p.usage == index.usage_map())
    {
        set_member(&mut promotion.bitmap, number, reader)?;
        global(image, number, &mut plan.append, reader)?;
        plan.append
            .reserve_index(number, index, reader.budget, reader.cancelled)?;
        return Ok(number);
    }

    reader.work(plan.rebases.len())?;
    if plan
        .rebases
        .iter()
        .any(|map| map.usage == index.usage_map())
    {
        rewrite_inline(image, index, number, plan, reader)?;
        return Ok(number);
    }

    let owned = RepresentedMap::read(reader, index.usage_map(), MapRole::Owned)?;
    match owned.future_cell(reader, number) {
        Ok(_) => {
            maps::allocate_nonroot(
                image,
                index,
                number,
                &mut plan.append,
                reader.budget,
                reader.cancelled,
            )?;
            plan.append
                .reserve_index(number, index, reader.budget, reader.cancelled)?;
            Ok(number)
        }
        Err(InspectError::Unsupported) if owned.kind() == INLINE_KIND => {
            rewrite_inline(image, index, number, plan, reader)?;
            Ok(number)
        }
        Err(error) => Err(error.into()),
    }
}

fn rewrite_inline(
    image: &NativeImage,
    index: &NativeIndex,
    number: u32,
    plan: &mut Plan,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    let usage = index.usage_map();
    // Rewrites use strict original role grammar as well as the exact row frame.
    // A previously staged rebase never supplies its own before-image authority.
    RepresentedMap::read(reader, usage, MapRole::Owned)?;
    let row = QualifiedInlineRecord::capture(reader, usage)?;
    let mut members = allocation::pages(reader, usage)?;
    reader.work(plan.append.reservations_len())?;
    for reservation in plan.append.indexes() {
        let prior_usage = reservation.usage;
        let number = reservation.number;
        reader.tick()?;
        if prior_usage == usage && reservation.physical == index.data_number() {
            reader
                .budget
                .charge_allocation(1, size_of::<u32>() as u64)
                .map_err(InspectError::from)?;
            members
                .try_reserve(1)
                .map_err(|_| InspectError::Allocation)?;
            members.push(number);
        }
    }
    reader.work(members.len().saturating_mul(2))?;
    let minimum = members
        .iter()
        .copied()
        .chain([number])
        .min()
        .ok_or(InspectError::Malformed)?;
    let maximum = members
        .iter()
        .copied()
        .chain([number])
        .max()
        .ok_or(InspectError::Malformed)?;
    let span = maximum
        .checked_sub(minimum)
        .and_then(|n| n.checked_add(1))
        .ok_or(InspectError::StructureLimit)?;
    reader.work(plan.rebases.len())?;
    let previous = plan.rebases.iter().position(|map| map.usage == usage);
    let inline_base = match plan.allocation_policy {
        AllocationPolicy::Legacy => {
            if span as usize <= QualifiedInlineRecord::PAGE_CAPACITY {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }
            None
        }
        AllocationPolicy::Qualified => {
            let current = if let Some(position) = previous {
                let map = &plan.rebases[position];
                if map.physical != index.data_number() {
                    return Err(EditError::Preservation);
                }
                map.base
            } else {
                row.start()?
            };
            let represented = number
                .checked_sub(current)
                .is_some_and(|relative| (relative as usize) < QualifiedInlineRecord::PAGE_CAPACITY);
            if represented {
                Some(current)
            } else {
                let aligned = minimum - minimum % u8::BITS;
                let aligned_span = maximum
                    .checked_sub(aligned)
                    .and_then(|span| span.checked_add(1))
                    .ok_or(InspectError::StructureLimit)?;
                (aligned_span < QualifiedInlineRecord::PAGE_CAPACITY as u32).then_some(aligned)
            }
        }
    };
    if inline_base.is_none() && maximum as usize >= BITMAP_CAPACITY {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }

    let owner = Owner::IndexOwned {
        definition: plan.append.owner,
        physical: index.data_number(),
    };
    plan.append.require_source_map_rewrite(
        image,
        &[Owner::GlobalFree, owner],
        RecordIdentity {
            owner,
            reference: row.reference(),
            range: row.range(),
        },
        reader,
    )?;

    if let Some(base) = inline_base {
        let global_map = RepresentedMap::read(reader, GLOBAL_FREE_REFERENCE, MapRole::GlobalFree)?;
        global_map.require_allocated_storage(reader, &global_map)?;
        let owned = RepresentedMap::read(reader, usage, MapRole::Owned)?;
        owned.require_allocated_storage(reader, &global_map)?;
        for &member in &members {
            reader.tick()?;
            if member < image.catalog().header().page_count() {
                global_map.require_allocated(reader, member)?;
            }
        }

        reader
            .budget
            .charge_allocation(1, size_of::<u32>() as u64)
            .map_err(InspectError::from)?;
        members
            .try_reserve(1)
            .map_err(|_| InspectError::Allocation)?;
        members.push(number);
        let after = row.inline(base, &members, reader)?;
        append::rewrite_record(
            &mut plan.append.map_edits,
            row.range().start,
            row.before(),
            &after,
            reader,
        )?;

        if let Some(position) = previous {
            plan.rebases[position].base = base;
        } else {
            reader
                .budget
                .charge_allocation(1, size_of::<Rebase>() as u64)
                .map_err(InspectError::from)?;
            plan.rebases
                .try_reserve(1)
                .map_err(|_| InspectError::Allocation)?;
            plan.rebases.push(Rebase {
                usage,
                physical: index.data_number(),
                base,
            });
        }

        global(image, number, &mut plan.append, reader)?;
        plan.append
            .reserve_index(number, index, reader.budget, reader.cancelled)?;

        return Ok(());
    }

    let metadata = plan.append.planned_number(1)?;
    plan.append.preview(2)?;
    let global_map = RepresentedMap::read(reader, GLOBAL_FREE_REFERENCE, MapRole::GlobalFree)?;
    global_map.require_allocated_storage(reader, &global_map)?;
    let owned = RepresentedMap::read(reader, usage, MapRole::Owned)?;
    owned.require_allocated_storage(reader, &global_map)?;
    for &member in &members {
        reader.tick()?;
        if member < image.catalog().header().page_count() {
            global_map.require_allocated(reader, member)?;
        }
    }
    reader
        .budget
        .charge_allocation(PAGE_BYTES as u64, 1)
        .map_err(InspectError::from)?;
    let mut bitmap = Vec::new();
    bitmap
        .try_reserve_exact(PAGE_BYTES)
        .map_err(|_| InspectError::Allocation)?;
    bitmap.resize(PAGE_BYTES, 0);
    bitmap[..USAGE_SIGNATURE.len()].copy_from_slice(&USAGE_SIGNATURE);
    for member in members.into_iter().chain([number]) {
        set_member(&mut bitmap, member, reader)?;
    }
    let after = row.indirect(metadata, reader)?;

    // Earlier children may have fit the old inline window. Their membership
    // now belongs to the complete promoted bitmap, never to the rewritten row.
    let record = row.range();
    reader.work(plan.append.map_edits.len())?;
    plan.append
        .map_edits
        .retain(|edit| !record.contains(&edit.offset));
    if let Some(position) = previous {
        reader.work(plan.rebases.len())?;
        plan.rebases.remove(position);
    }

    global(image, number, &mut plan.append, reader)?;
    global(image, metadata, &mut plan.append, reader)?;
    reader
        .budget
        .charge_allocation(1, size_of::<Promotion>() as u64)
        .map_err(InspectError::from)?;
    plan.promotions
        .try_reserve(1)
        .map_err(|_| InspectError::Allocation)?;
    plan.promotions.push(Promotion {
        usage,
        number: metadata,
        offset: record.start,
        before: row.into_before(),
        after,
        bitmap,
    });
    let actual =
        plan.append
            .reserve_index_bitmap(number, index, reader.budget, reader.cancelled)?;
    if actual != metadata {
        return Err(EditError::Preservation);
    }

    Ok(())
}

fn set_member(
    bitmap: &mut [u8],
    number: u32,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    reader.tick()?;
    if number as usize >= BITMAP_CAPACITY {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }

    let byte = BITMAP_PREFIX_BYTES + number as usize / u8::BITS as usize;
    let mask = 1 << (number % u8::BITS);
    if bitmap[byte] & mask != 0 {
        return Err(InspectError::Malformed.into());
    }
    bitmap[byte] |= mask;

    Ok(())
}

fn global(
    image: &NativeImage,
    number: u32,
    append: &mut append::State,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    let mut source = image.reader();
    let mut original = Reader {
        source: &mut source,
        header: image.catalog().header(),
        budget: reader.budget,
        cancelled: reader.cancelled,
        reads: 0,
    };
    let global = RepresentedMap::read(&mut original, GLOBAL_FREE_REFERENCE, MapRole::GlobalFree)?;
    if !append.global_is_free(&global, number, &mut original)? {
        return Err(InspectError::Malformed.into());
    }
    append.claim_global(&global, number, &mut original)
}

pub(super) fn seal(
    image: &NativeImage,
    indexes: &[IndexCheck],
    plan: &mut Plan,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    if !plan.promotions.is_empty() || !plan.rebases.is_empty() {
        let mut owners = Vec::new();
        reader
            .budget
            .charge_allocation(
                (plan.append.reservations_len() + 1) as u64,
                size_of::<Owner>() as u64,
            )
            .map_err(InspectError::from)?;
        owners
            .try_reserve(plan.append.reservations_len() + 1)
            .map_err(|_| InspectError::Allocation)?;
        owners.push(Owner::GlobalFree);
        reader.work(plan.append.reservations_len())?;
        for reservation in plan.append.indexes() {
            let usage = reservation.usage;
            reader.tick()?;
            reader.work(indexes.len() + owners.len())?;
            let index = indexes
                .iter()
                .find(|checked| checked.index().usage_map() == usage)
                .ok_or(EditError::Preservation)?;
            if index.index().data_number() != reservation.physical {
                return Err(EditError::Preservation);
            }
            let owner = Owner::IndexOwned {
                definition: plan.append.owner,
                physical: index.index().data_number(),
            };
            if !owners.contains(&owner) {
                owners.push(owner);
            }
        }
        plan.append
            .require_source_map_exclusive(image, &owners, reader)?;
    }

    for promotion in &plan.promotions {
        reader.tick()?;
        reader.work(indexes.len())?;
        let physical = indexes
            .iter()
            .find(|checked| checked.index().usage_map() == promotion.usage)
            .ok_or(EditError::Preservation)?
            .index()
            .data_number();
        plan.append.require_index_bitmap(
            promotion.number,
            physical,
            promotion.usage,
            reader.budget,
            reader.cancelled,
        )?;
        reader
            .budget
            .charge_allocation(PAGE_BYTES as u64, 1)
            .map_err(InspectError::from)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(PAGE_BYTES)
            .map_err(|_| InspectError::Allocation)?;
        bytes.extend_from_slice(&promotion.bitmap);
        reader
            .budget
            .charge_allocation(1, size_of::<AddedPage>() as u64)
            .map_err(InspectError::from)?;
        plan.added
            .try_reserve(1)
            .map_err(|_| InspectError::Allocation)?;
        plan.added.push(AddedPage {
            number: promotion.number,
            bytes,
        });
    }
    reader.work(plan.added.len().saturating_mul(plan.added.len()))?;
    plan.added.sort_unstable_by_key(|page| page.number);
    Ok(())
}

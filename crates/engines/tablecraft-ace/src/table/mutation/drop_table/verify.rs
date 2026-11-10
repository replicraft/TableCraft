use super::{TableDropError, plan::offset, source::Qualified};
use crate::{
    InspectError, NativeImage,
    format::{
        bytes::{u16_at, u32_at},
        layout::RECORD_COUNT_OFFSET,
        row::{DELETED_MASK, OFFSET_MASK, RELOCATED_MASK, SLOT_BYTES, slot_offset},
    },
    header::check,
    inspect_catalog,
};
use std::{io::Cursor, ops::Range};
use tablecraft_core::limits::{Resource, ResourceBudget};

const TOMBSTONE_FLAGS: u16 = DELETED_MASK | RELOCATED_MASK;
const COMPARISON_CHUNK_BYTES: usize = 64 * 1024;

struct Scope {
    slot: Range<usize>,
    live_count: Range<usize>,
    original_slot: u16,
    original_count: u32,
}

impl Scope {
    fn capture(before: &NativeImage, source: &Qualified) -> Result<Self, TableDropError> {
        let slot = offset(source.identity.catalog_page())?
            .checked_add(slot_offset(usize::from(source.identity.catalog_slot())))
            .ok_or(InspectError::StructureLimit)?;
        let live_count = offset(source.catalog_definition)?
            .checked_add(RECORD_COUNT_OFFSET)
            .ok_or(InspectError::StructureLimit)?;
        let original_slot = u16_at(before.as_bytes(), slot)?;
        let original_count = u32_at(before.as_bytes(), live_count)?;
        if original_slot & !OFFSET_MASK != 0
            || original_count == 0
            || original_count != source.catalog_rows
        {
            return Err(TableDropError::Source);
        }

        Ok(Self {
            slot: slot..slot + SLOT_BYTES,
            live_count: live_count..live_count + size_of::<u32>(),
            original_slot,
            original_count,
        })
    }

    fn contains(&self, position: usize) -> bool {
        self.slot.contains(&position) || self.live_count.contains(&position)
    }
}

pub(super) fn write(
    before: &NativeImage,
    after: &mut [u8],
    source: &Qualified,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), TableDropError> {
    check(budget, cancelled)?;
    let scope = Scope::capture(before, source)?;
    budget
        .charge(Resource::Steps, (SLOT_BYTES + size_of::<u32>()) as u64)
        .map_err(InspectError::from)?;
    after
        .get_mut(scope.slot)
        .ok_or(InspectError::Truncated)?
        .copy_from_slice(&(scope.original_slot | TOMBSTONE_FLAGS).to_le_bytes());
    after
        .get_mut(scope.live_count)
        .ok_or(InspectError::Truncated)?
        .copy_from_slice(&(scope.original_count - 1).to_le_bytes());
    check(budget, cancelled)?;
    Ok(())
}

pub(super) fn candidate(
    before: &NativeImage,
    after: &[u8],
    source: &Qualified,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), TableDropError> {
    check(budget, cancelled)?;
    if before.as_bytes().len() != after.len() {
        return Err(TableDropError::Preservation);
    }
    let scope = Scope::capture(before, source)?;
    if u16_at(after, scope.slot.start)? != (scope.original_slot | TOMBSTONE_FLAGS)
        || u32_at(after, scope.live_count.start)? != scope.original_count - 1
    {
        return Err(TableDropError::Preservation);
    }

    for (chunk_index, (original, changed)) in before
        .as_bytes()
        .chunks(COMPARISON_CHUNK_BYTES)
        .zip(after.chunks(COMPARISON_CHUNK_BYTES))
        .enumerate()
    {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, original.len() as u64)
            .map_err(InspectError::from)?;
        let base = chunk_index * COMPARISON_CHUNK_BYTES;
        for (position, (old, new)) in original.iter().zip(changed).enumerate() {
            if old != new && !scope.contains(base + position) {
                return Err(TableDropError::Preservation);
            }
        }
    }

    catalog(before, after, source, budget, cancelled)
}

pub(super) fn catalog(
    before: &NativeImage,
    after: &[u8],
    source: &Qualified,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), TableDropError> {
    let fresh = inspect_catalog(
        &mut Cursor::new(after),
        before.catalog().catalog().scope(),
        budget,
        cancelled,
    )?;
    if before.catalog().header() != fresh.header()
        || before.catalog().origins().len().checked_sub(1) != Some(fresh.origins().len())
    {
        return Err(TableDropError::Preservation);
    }

    let mut remaining = fresh.catalog().objects().iter().zip(fresh.origins());
    let mut removed = false;
    for (old, origin) in before
        .catalog()
        .catalog()
        .objects()
        .iter()
        .zip(before.catalog().origins())
    {
        check(budget, cancelled)?;
        if old.id() == source.identity.object_id() {
            if removed
                || old.name().as_str() != source.table.as_str()
                || origin.native_id() != source.identity.native_id()
                || origin.parent_id() != source.identity.parent_id()
                || origin.page() != source.identity.catalog_page()
                || origin.slot() != source.identity.catalog_slot()
            {
                return Err(TableDropError::Preservation);
            }
            removed = true;
            continue;
        }

        let Some((new, new_origin)) = remaining.next() else {
            return Err(TableDropError::Preservation);
        };
        budget
            .charge(
                Resource::Steps,
                old.name().as_str().len().max(new.name().as_str().len()) as u64,
            )
            .map_err(InspectError::from)?;
        if old.id() != new.id()
            || old.name() != new.name()
            || old.kind() != new.kind()
            || old.parent() != new.parent()
            || origin.native_id() != new_origin.native_id()
            || origin.parent_id() != new_origin.parent_id()
            || origin.type_code() != new_origin.type_code()
            || origin.flags() != new_origin.flags()
            || origin.page() != new_origin.page()
            || origin.slot() != new_origin.slot()
        {
            return Err(TableDropError::Preservation);
        }
    }
    if !removed || remaining.next().is_some() {
        return Err(TableDropError::Preservation);
    }
    check(budget, cancelled)?;
    Ok(())
}

//! Shared bounded image mechanics; source admission stays with each profile.
use crate::{
    InspectError, NativeImage,
    format::{definition, layout, page::PAGE_BYTES},
    header::check,
};
use std::ops::Range;
use tablecraft_core::limits::{Resource, ResourceBudget};

const CHUNK_BYTES: usize = 16 * 1024;
const FREE_BYTES: Range<usize> = 2..4;
const DEFINITION_LENGTH: Range<usize> =
    definition::LENGTH_OFFSET..definition::LENGTH_OFFSET + size_of::<u32>();
const MAX_COLUMNS: Range<usize> =
    layout::MAX_COLUMN_COUNT_OFFSET..layout::MAX_COLUMN_COUNT_OFFSET + size_of::<u16>();
const LIVE_COLUMNS: Range<usize> =
    layout::COLUMN_COUNT_OFFSET..layout::COLUMN_COUNT_OFFSET + size_of::<u16>();

pub(super) fn equal_image(
    actual: &[u8],
    expected: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, InspectError> {
    if actual.len() != expected.len() {
        return Ok(false);
    }
    for (actual, expected) in actual.chunks(CHUNK_BYTES).zip(expected.chunks(CHUNK_BYTES)) {
        check(budget, cancelled)?;
        budget.charge(Resource::Steps, actual.len() as u64)?;
        if actual != expected {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(super) fn copy_definition(
    before: &NativeImage,
    reconstructed: &[u8],
    definition: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, InspectError> {
    budget.charge_allocation(before.as_bytes().len() as u64, 1)?;
    let mut after = Vec::new();
    after
        .try_reserve_exact(before.as_bytes().len())
        .map_err(|_| InspectError::Allocation)?;
    for chunk in before.as_bytes().chunks(CHUNK_BYTES) {
        check(budget, cancelled)?;
        budget.charge(Resource::Steps, chunk.len() as u64)?;
        after.extend_from_slice(chunk);
    }
    let end = definition + PAGE_BYTES;
    let target = after
        .get_mut(definition..end)
        .ok_or(InspectError::Malformed)?;
    let source = reconstructed
        .get(definition..end)
        .ok_or(InspectError::Malformed)?;
    target.copy_from_slice(source);
    Ok(after)
}

pub(super) fn preserves_complement(
    before: &[u8],
    after: &[u8],
    definition: usize,
    declaration_tail: Range<usize>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, InspectError> {
    if before.len() != after.len() || declaration_tail.end > PAGE_BYTES {
        return Ok(false);
    }
    for (at, (original, proposed)) in before.iter().zip(after).enumerate() {
        if at % CHUNK_BYTES == 0 {
            check(budget, cancelled)?;
        }
        budget.charge(Resource::Steps, 1)?;
        let offset = at.wrapping_sub(definition);
        let allowed = FREE_BYTES.contains(&offset)
            || DEFINITION_LENGTH.contains(&offset)
            || MAX_COLUMNS.contains(&offset)
            || LIVE_COLUMNS.contains(&offset)
            || declaration_tail.contains(&offset);
        if original != proposed && !allowed {
            return Ok(false);
        }
    }
    Ok(true)
}

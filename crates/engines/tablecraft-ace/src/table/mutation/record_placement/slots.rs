//! Physical placement directory ranges, including bounded empty deleted slots.
use crate::{
    EditError, InspectError,
    format::{
        bytes::u16_at,
        reader::row_count,
        row::{DELETED_MASK, OFFSET_MASK, UNSUPPORTED_MASK, slot_offset},
    },
    header::check,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const RANGE_WORD_WORK: u64 = 3 * size_of::<u16>() as u64;

/// Match Reader's word masks and bounds, allowing only deleted empty extents.
/// A live empty slot never becomes a logical row through this placement helper.
pub(super) fn range(page: &[u8], slot: usize) -> Result<(u16, usize, usize), InspectError> {
    let count = row_count(page)?;
    if slot >= count {
        return Err(InspectError::Malformed);
    }
    let raw = u16_at(page, slot_offset(slot))?;
    if raw & UNSUPPORTED_MASK != 0 {
        return Err(InspectError::Unsupported);
    }
    let start = usize::from(raw & OFFSET_MASK);
    let end = if slot == 0 {
        page.len()
    } else {
        usize::from(u16_at(page, slot_offset(slot - 1))? & OFFSET_MASK)
    };
    if start < slot_offset(count)
        || start > end
        || end > page.len()
        || (start == end && raw & DELETED_MASK == 0)
    {
        return Err(InspectError::Malformed);
    }
    Ok((raw, start, end))
}

/// Check every physical slot before choosing a placement frontier.
pub(in crate::table::mutation) fn frontier(
    page: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<usize, EditError> {
    check(budget, cancelled)?;
    let count = row_count(page)?;
    let mut end = page.len();
    for slot in 0..count {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, RANGE_WORD_WORK)
            .map_err(InspectError::from)?;
        end = range(page, slot)?.1;
    }
    check(budget, cancelled)?;
    Ok(end)
}

#[cfg(test)]
mod tests;

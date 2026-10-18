use super::EditError;
use crate::{InspectError, header::check, preservation::COPY_CHUNK_BYTES};
use std::ops::Range;
use tablecraft_core::limits::{Resource, ResourceBudget};

// Only exact field and encoded-entry spans are admitted. Index headers, prefix
// declarations, bitmaps, padding and all other opaque content remain outside.
pub(super) fn prove(
    before: &[u8],
    after: &[u8],
    mut spans: Vec<Range<usize>>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    if before.len() != after.len() {
        return Err(EditError::Preservation);
    }
    let levels = u64::from(spans.len().checked_ilog2().unwrap_or(0)) + 1;
    budget
        .charge(Resource::Steps, spans.len() as u64 * levels)
        .map_err(InspectError::from)?;
    spans.sort_unstable_by_key(|span| span.start);
    let mut previous = 0;
    for span in spans {
        check(budget, cancelled)?;
        if span.start < previous || span.start > span.end || span.end > before.len() {
            return Err(EditError::Preservation);
        }
        equal(
            &before[previous..span.start],
            &after[previous..span.start],
            budget,
            cancelled,
        )?;
        previous = span.end;
    }
    equal(&before[previous..], &after[previous..], budget, cancelled)?;
    check(budget, cancelled)?;
    Ok(())
}
fn equal(
    before: &[u8],
    after: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    for (left, right) in before
        .chunks(COPY_CHUNK_BYTES)
        .zip(after.chunks(COPY_CHUNK_BYTES))
    {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, left.len() as u64)
            .map_err(InspectError::from)?;
        if left != right {
            return Err(EditError::Preservation);
        }
    }
    Ok(())
}

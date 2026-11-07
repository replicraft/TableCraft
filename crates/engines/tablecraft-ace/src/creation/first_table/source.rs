//! The one complete-byte source predicate shared by availability and planning.
use super::FirstTableError;
use crate::{InspectError, create_blank_ace16_database, header::check};
use tablecraft_core::limits::{Resource, ResourceBudget};

const COMPARE_BLOCK_BYTES: usize = 16 * 1024;

/// Inspect an already captured immutable source for the exact canonical blank.
/// This constructs only the bounded canonical comparison image. It does not
/// parse a proposed declaration, create a draft, write files or grant permission
/// to apply an edit. Planning and actual target preparation recheck their source.
/// Only a complete source mismatch returns false; resource, cancellation and
/// canonical construction failures remain errors.
pub fn first_table_source_available(
    captured: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, FirstTableError> {
    let _span = tracing::debug_span!("native.first_table.source_available").entered();
    match require_canonical_blank(captured, budget, cancelled) {
        Ok(()) => Ok(true),
        Err(FirstTableError::NotCanonicalBlank) => Ok(false),
        Err(error) => Err(error),
    }
}

pub(super) fn require_canonical_blank(
    captured: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), FirstTableError> {
    check(budget, cancelled)?;
    let canonical = create_blank_ace16_database(budget, cancelled)?;
    if captured.len() != canonical.as_bytes().len() {
        return Err(FirstTableError::NotCanonicalBlank);
    }
    for (actual, expected) in captured
        .chunks(COMPARE_BLOCK_BYTES)
        .zip(canonical.as_bytes().chunks(COMPARE_BLOCK_BYTES))
    {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, actual.len() as u64)
            .map_err(InspectError::from)?;
        if actual != expected {
            return Err(FirstTableError::NotCanonicalBlank);
        }
    }
    check(budget, cancelled)?;
    Ok(())
}

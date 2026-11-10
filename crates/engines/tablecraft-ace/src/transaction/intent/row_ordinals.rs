//! Paid sparse duplicate detection for original request row ordinals.
use super::NativeIntentError;
use crate::{EditError, InspectError, header::check};
use std::collections::HashSet;
use tablecraft_core::limits::{Resource, ResourceBudget};

// Logical accounting includes hash metadata and spare capacity. It depends on
// actual entries and does not claim to measure exact allocator residency.
const IDENTITY_ALLOCATION_BYTES: u64 = 4 * size_of::<usize>() as u64;

#[derive(Default)]
pub(super) struct RowOrdinals {
    seen: HashSet<usize>,
}

impl RowOrdinals {
    pub(super) fn insert(
        &mut self,
        row: usize,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), NativeIntentError> {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, 1)
            .map_err(InspectError::from)?;
        if self.seen.contains(&row) {
            return Err(EditError::Conflict.into());
        }
        budget
            .charge_allocation(1, IDENTITY_ALLOCATION_BYTES)
            .map_err(InspectError::from)?;
        self.seen
            .try_reserve(1)
            .map_err(|_| InspectError::Allocation)?;
        self.seen.insert(row);
        Ok(())
    }
}

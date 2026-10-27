//! Original request positions, sorted by live-row ordinal for a monotone walk.
use super::{EditError, RowAssignment};
use crate::{InspectError, header::check, limits::MAX_LIVE_ROWS};
use tablecraft_core::limits::{Resource, ResourceBudget};

#[cfg(test)]
mod tests;

#[derive(Clone, Copy)]
struct Entry {
    row: usize,
    request: usize,
}

pub(in crate::table::mutation) struct Directory {
    entries: Vec<Entry>,
    next: usize,
}

impl Directory {
    pub(in crate::table::mutation) fn new<U: RowAssignment>(
        requests: &[U],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        if requests.len() > MAX_LIVE_ROWS {
            return Err(InspectError::StructureLimit.into());
        }
        check(budget, cancelled)?;
        budget
            .charge_allocation(requests.len() as u64, size_of::<Entry>() as u64)
            .map_err(InspectError::from)?;
        let mut entries: Vec<Entry> = Vec::new();
        entries
            .try_reserve_exact(requests.len())
            .map_err(|_| InspectError::Allocation)?;
        let mut ordered = true;
        for (request, value) in requests.iter().enumerate() {
            check(budget, cancelled)?;
            let row = value.row();
            if let Some(previous) = entries.last() {
                ordered &= previous.row <= row;
            }
            entries.push(Entry { row, request });
        }

        // Generated ordinal captures are already ordered. Only unsorted input
        // pays for sorting; the original position breaks duplicate ties.
        if !ordered {
            let levels = requests.len().checked_ilog2().unwrap_or(0) as u64 + 1;
            budget
                .charge(Resource::Steps, requests.len() as u64 * levels)
                .map_err(InspectError::from)?;
            entries.sort_unstable_by_key(|entry| (entry.row, entry.request));
            check(budget, cancelled)?;
        }

        Ok(Self { entries, next: 0 })
    }

    pub(in crate::table::mutation) fn require_unique(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        for pair in self.entries.windows(2) {
            check(budget, cancelled)?;
            if pair[0].row == pair[1].row {
                return Err(EditError::Conflict);
            }
        }
        Ok(())
    }

    /// Calls must follow strictly increasing physical live-row ordinals.
    /// Ties retain the first original request, as the previous linear find did.
    /// The walker still compares its matched count with the original request
    /// count, so missing and duplicate requests cannot become successful reads.
    pub(in crate::table::mutation) fn at(
        &mut self,
        ordinal: usize,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Option<usize>, EditError> {
        check(budget, cancelled)?;
        while let Some(entry) = self.entries.get(self.next) {
            if entry.row >= ordinal {
                break;
            }
            check(budget, cancelled)?;
            self.next += 1;
        }

        let Some(entry) = self.entries.get(self.next) else {
            return Ok(None);
        };
        if entry.row != ordinal {
            return Ok(None);
        }

        self.next += 1;
        Ok(Some(entry.request))
    }
}

//! Shared TOP selection over complete, already ordered result rows.
use super::{Runtime, retrieval::OutputRow};
use crate::{QueryError, plan::ReadPlan, profile::PERCENT_TOTAL};
use std::cmp::Ordering;

impl Runtime<'_, '_> {
    /// Apply the bound positive count/percentage and retain complete ORDER BY
    /// ties. Callers must finish evaluation and sorting before discarding rows:
    /// TOP is not permission to hide errors in an omitted row or crosstab cell.
    pub(super) fn apply_top(
        &mut self,
        rows: &mut Vec<OutputRow>,
        plan: &ReadPlan,
    ) -> Result<(), QueryError> {
        let Some((count, percent)) = plan.top else {
            return Ok(());
        };
        self.tick()?;
        let mut take = if percent {
            (rows.len() as u64 * u64::from(count)).div_ceil(u64::from(PERCENT_TOTAL)) as usize
        } else {
            count as usize
        };
        if take > 0 && !plan.sort.is_empty() {
            while take < rows.len()
                && self.compare_keys(&rows[take - 1].keys, &rows[take].keys, plan)?
                    == Ordering::Equal
            {
                take += 1;
            }
        }
        rows.truncate(take);
        self.tick()
    }
}

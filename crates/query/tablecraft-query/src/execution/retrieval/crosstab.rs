//! Crosstab output has already completed its own sorting and TOP work.
use super::Runtime;
use crate::{
    QueryError,
    execution::cell::CellValue,
    plan::{CrosstabPlan, ReadPlan},
};
use tablecraft_core::limits::Resource;

impl Runtime<'_, '_> {
    #[inline(never)]
    pub(super) fn read_crosstab(
        &mut self,
        cross: &CrosstabPlan,
        plan: &ReadPlan,
    ) -> Result<Vec<Vec<CellValue>>, QueryError> {
        let result = self.crosstab_in_read(cross)?;
        if result.columns.len() != plan.names.len() {
            return Err(QueryError::SchemaChanged);
        }
        for (actual, expected) in result.columns.iter().zip(&plan.names) {
            self.tick()?;
            self.budget
                .charge(Resource::Steps, (actual.len() + expected.len()) as u64)?;
            if actual != expected {
                return Err(QueryError::SchemaChanged);
            }
        }
        // Inner crosstab already evaluated every cell, sorted and
        // applied its TOP. Outer consumers retain all resulting cells.
        Ok(result.rows)
    }
}

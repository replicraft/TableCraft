//! Explicit borrowed projection over already validated in-memory rows.
use super::MemorySource;
use crate::QueryError;
use tablecraft_db::{ProjectedRow, validate_projection};

impl MemorySource {
    pub(super) fn progressive_projection(
        &self,
        table: &str,
        indices: &[usize],
        visitor: &mut dyn FnMut(ProjectedRow<'_>) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        let table = self.table(table)?;
        validate_projection(indices, table.columns.len())?;
        for row in &table.rows {
            visitor(ProjectedRow::select(row, indices)?)?;
        }
        Ok(())
    }
}

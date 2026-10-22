use super::Runtime;
use crate::{OutputType, QueryError};
use tablecraft_core::values::{ExpressionValue, Value};

/// Fully buffered result, with no Debug/Serde payload exposure.
pub struct QueryResult<T = Value> {
    pub(super) columns: Vec<String>,
    pub(super) output_types: Vec<OutputType>,
    pub(super) rows: Vec<Vec<T>>,
}
/// Query data that may include non-storable expression states.
pub type ExpressionQueryResult = QueryResult<ExpressionValue>;

impl<T> QueryResult<T> {
    /// Ordered output names, preserving aliases and possible duplicates.
    pub fn columns(&self) -> &[String] {
        &self.columns
    }
    /// Conservative scalar metadata aligned with output names. Dynamic entries
    /// retain unknown/varying carriers; no type is inferred from returned rows.
    pub fn output_types(&self) -> &[OutputType] {
        &self.output_types
    }
    /// Materialized rows; no storage cursor/updateability is implied.
    pub fn rows(&self) -> &[Vec<T>] {
        &self.rows
    }
}

impl Runtime<'_, '_> {
    pub(super) fn copy_output_types(
        &mut self,
        types: &[OutputType],
    ) -> Result<Vec<OutputType>, QueryError> {
        self.budget
            .charge_allocation(types.len() as u64, size_of::<OutputType>() as u64)?;
        let mut result = Vec::new();
        result
            .try_reserve_exact(types.len())
            .map_err(|_| QueryError::Allocation)?;
        result.extend_from_slice(types);
        Ok(result)
    }
}

use tablecraft_core::values::{ExpressionValue, Value};

/// Fully buffered result, with no Debug/Serde payload exposure.
pub struct QueryResult<T = Value> {
    pub(super) columns: Vec<String>,
    pub(super) rows: Vec<Vec<T>>,
}
/// Query data that may include non-storable expression states.
pub type ExpressionQueryResult = QueryResult<ExpressionValue>;

impl<T> QueryResult<T> {
    /// Ordered output names, preserving aliases and possible duplicates.
    pub fn columns(&self) -> &[String] {
        &self.columns
    }
    /// Materialized rows; no storage cursor/updateability is implied.
    pub fn rows(&self) -> &[Vec<T>] {
        &self.rows
    }
}

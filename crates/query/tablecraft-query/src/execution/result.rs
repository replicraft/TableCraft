use tablecraft_core::values::Value;

/// Fully buffered result, with no Debug/Serde payload exposure.
pub struct QueryResult {
    pub(super) columns: Vec<String>,
    pub(super) rows: Vec<Vec<Value>>,
}
impl QueryResult {
    /// Ordered output names, preserving aliases and possible duplicates.
    pub fn columns(&self) -> &[String] {
        &self.columns
    }
    /// Materialized rows; no storage cursor/updateability is implied.
    pub fn rows(&self) -> &[Vec<Value>] {
        &self.rows
    }
}

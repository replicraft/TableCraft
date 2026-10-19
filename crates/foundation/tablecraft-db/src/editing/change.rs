use crate::RowHandle;
use tablecraft_core::values::Value;

/// Replacement for one snapshot-issued row in a single-column update batch.
/// No Debug/Serde exposes the replacement value.
pub struct CellUpdate {
    row: RowHandle,
    value: Value,
}
impl CellUpdate {
    /// Bind a replacement to an exact snapshot row.
    pub const fn new(row: RowHandle, value: Value) -> Self {
        Self { row, value }
    }
    /// Issuing snapshot row handle.
    pub const fn row(&self) -> RowHandle {
        self.row
    }
    /// Exact requested replacement carrier; backends must not silently coerce it.
    pub fn value(&self) -> &Value {
        &self.value
    }
}

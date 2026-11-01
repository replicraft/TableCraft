//! Explicit complete row data shared by normal and engineering insertion.
use tablecraft_core::values::Value;

/// One complete new row in the captured native column order.
/// Values are neither defaulted nor coerced; the selected insertion profile
/// validates every carrier against the actual table before constructing bytes.
pub struct RowInsert {
    values: Vec<Value>,
}

impl RowInsert {
    /// Capture all explicit field values. Table-dependent admission happens later.
    pub fn new(values: Vec<Value>) -> Self {
        Self { values }
    }

    /// Explicit requested values, in native column order.
    pub fn values(&self) -> &[Value] {
        &self.values
    }
}

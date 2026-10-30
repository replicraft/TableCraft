//! Explicit generated-field omission and bounded copying of companion carriers.
use tablecraft_core::values::Value;

/// Caller-selected incrementing engineering behavior and source precondition.
///
/// This does not detect or certify the source's stored New Values mode. There
/// is no default policy, random generation, explicit-ID override or reseeding.
#[derive(Clone, Copy)]
pub struct IncrementingAutoNumber {
    expected_last: i32,
}

impl IncrementingAutoNumber {
    /// Pin the last issued Int32 value in the original table-definition counter.
    pub fn new(expected_last: i32) -> Self {
        Self { expected_last }
    }

    /// Exact expected original counter, not the first generated value.
    pub fn expected_last(self) -> i32 {
        self.expected_last
    }
}

/// Explicit companion values in native column order, excluding the one
/// schema-resolved generated primary field. NULL remains a stored value.
pub struct AutoNumberRowInsert {
    values: Vec<Value>,
}

impl AutoNumberRowInsert {
    /// Capture companions; no value acts as a request to generate a field.
    pub fn new(values: Vec<Value>) -> Self {
        Self { values }
    }

    /// Companion values, excluding the generated column.
    pub fn values(&self) -> &[Value] {
        &self.values
    }
}

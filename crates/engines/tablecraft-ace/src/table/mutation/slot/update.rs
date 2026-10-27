use super::super::rows::RowAssignment;
use tablecraft_core::values::Value;

/// One exact expected-value assignment to an existing variable-field slot.
/// Values are representational preconditions, not SQL comparisons. Payloads are
/// deliberately excluded from Debug and tracing.
pub struct SlotScalarUpdate {
    row: usize,
    expected: Value,
    value: Value,
}

impl SlotScalarUpdate {
    /// Assign one snapshot-local live-row ordinal with exact old/new values.
    pub fn new(row: usize, expected: Value, value: Value) -> Self {
        Self {
            row,
            expected,
            value,
        }
    }

    /// Snapshot-local ordinal in the complete captured table's live row order.
    pub fn row(&self) -> usize {
        self.row
    }

    /// Exact representational value required in the original captured row.
    pub fn expected(&self) -> &Value {
        &self.expected
    }

    /// Replacement value; stored NULL and present empty values remain distinct.
    pub fn value(&self) -> &Value {
        &self.value
    }
}

impl RowAssignment for SlotScalarUpdate {
    fn row(&self) -> usize {
        self.row
    }
}

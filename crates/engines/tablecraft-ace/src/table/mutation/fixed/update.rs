use super::super::physical::FixedAssignment;
use tablecraft_core::values::Value;

/// An exact expected/replacement carrier for one snapshot-local live row.
///
/// The target native column determines eligibility. No SQL coercion, NULL
/// transition or variable-length encoding is implied. Values are deliberately
/// excluded from Debug/Display implementations.
pub struct FixedScalarUpdate {
    row: usize,
    expected: Value,
    value: Value,
}
impl FixedScalarUpdate {
    /// Address a live row of the captured image with exact expected/replacement values.
    pub fn new(row: usize, expected: Value, value: Value) -> Self {
        Self {
            row,
            expected,
            value,
        }
    }
    /// Sequential live-row ordinal in this snapshot only.
    pub const fn row(&self) -> usize {
        self.row
    }
    /// Original carrier, compared without numeric or temporal normalization.
    pub fn expected(&self) -> &Value {
        &self.expected
    }
    /// Replacement carrier; it must exactly match the qualified field type.
    pub fn value(&self) -> &Value {
        &self.value
    }
}

pub(super) struct EncodedUpdate<const WIDTH: usize> {
    pub row: usize,
    pub expected: [u8; WIDTH],
    pub value: [u8; WIDTH],
}
impl<const WIDTH: usize> FixedAssignment<WIDTH> for EncodedUpdate<WIDTH> {
    fn row(&self) -> usize {
        self.row
    }
    fn expected(&self) -> [u8; WIDTH] {
        self.expected
    }
    fn value(&self) -> [u8; WIDTH] {
        self.value
    }
}

use crate::{InspectError, format::layout::INDEX_KEY_COLUMNS};
use tablecraft_core::values::Guid;

pub(in crate::table::indexes) trait ComponentValue:
    Copy + Eq + Ord
{
    const GUID_ALLOWED: bool;
    const DECIMAL_ALLOWED: bool = false;
    fn empty() -> Self;
    fn numeric(value: i64) -> Self;
    fn guid(value: [u8; Guid::BYTE_LEN]) -> Result<Self, InspectError>;
    fn decimal(_value: i128) -> Result<Self, InspectError> {
        Err(InspectError::Unsupported)
    }
}
impl ComponentValue for i64 {
    const GUID_ALLOWED: bool = false;
    fn empty() -> Self {
        0
    }
    fn numeric(value: i64) -> Self {
        value
    }
    fn guid(_value: [u8; Guid::BYTE_LEN]) -> Result<Self, InspectError> {
        Err(InspectError::Unsupported)
    }
}

// Each position has a fixed qualified type. Cross-variant order is never used
// to compare different field types and does not supply SQL/Value comparison.
#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub(in crate::table::indexes) enum MixedValue {
    Numeric(i64),
    Guid([u8; Guid::BYTE_LEN]),
}
impl ComponentValue for MixedValue {
    const GUID_ALLOWED: bool = true;
    fn empty() -> Self {
        Self::Numeric(0)
    }
    fn numeric(value: i64) -> Self {
        Self::Numeric(value)
    }
    fn guid(value: [u8; Guid::BYTE_LEN]) -> Result<Self, InspectError> {
        Ok(Self::Guid(value))
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(in crate::table::indexes) struct CompositeKey<A> {
    values: [A; INDEX_KEY_COLUMNS],
    present: u16,
}
impl<A: ComponentValue> CompositeKey<A> {
    pub(super) fn empty() -> Self {
        Self {
            values: [A::empty(); INDEX_KEY_COLUMNS],
            present: 0,
        }
    }
    pub(in crate::table::indexes) fn set(&mut self, position: usize, value: Option<A>) {
        if let Some(value) = value {
            self.values[position] = value;
            self.present |= 1 << position;
        } else {
            self.values[position] = A::empty();
            self.present &= !(1 << position);
        }
    }
    pub(in crate::table::indexes) fn get(&self, position: usize) -> Option<A> {
        (self.present & (1 << position) != 0).then_some(self.values[position])
    }
    pub(super) fn all_null(&self) -> bool {
        self.present == 0
    }
    pub(super) fn any_null(&self, count: usize) -> bool {
        self.present != (1 << count) - 1
    }
}

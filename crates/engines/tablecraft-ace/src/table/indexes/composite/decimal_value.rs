//! Wider atoms are used only when a tuple actually contains a Decimal field.
use super::model::ComponentValue;
use crate::InspectError;
use tablecraft_core::values::Guid;

// A position has one declared type. Cross-variant order cannot compare different
// columns/types and does not supply SQL Value equality, ordering or coercion.
#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub(in crate::table::indexes) enum DecimalValue {
    Numeric(i64),
    Guid([u8; Guid::BYTE_LEN]),
    Decimal(i128),
}
impl ComponentValue for DecimalValue {
    const GUID_ALLOWED: bool = true;
    const DECIMAL_ALLOWED: bool = true;
    fn empty() -> Self {
        Self::Numeric(0)
    }
    fn numeric(value: i64) -> Self {
        Self::Numeric(value)
    }
    fn guid(value: [u8; Guid::BYTE_LEN]) -> Result<Self, InspectError> {
        Ok(Self::Guid(value))
    }
    fn decimal(value: i128) -> Result<Self, InspectError> {
        Ok(Self::Decimal(value))
    }
}

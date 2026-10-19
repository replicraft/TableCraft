use super::super::schema::TableSchema;
use super::{NativeIndex, constraints::IGNORE_NULLS};
use crate::InspectError;
use tablecraft_core::values::Value;
use tablecraft_schema::ScalarType;

// Distinct semantic types may share an encoded width. Typed ranges must still
// reject Currency when the caller requests an Int64 column.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum KeyType {
    Byte,
    Int16,
    Int32,
    Int64,
    Currency,
}
impl KeyType {
    pub const fn bytes(self) -> usize {
        match self {
            Self::Byte => size_of::<u8>(),
            Self::Int16 => size_of::<i16>(),
            Self::Int32 => size_of::<i32>(),
            Self::Int64 | Self::Currency => size_of::<i64>(),
        }
    }
    // One qualified carrier per physical index. Currency keeps its exact scaled
    // integer without becoming an Int64 column or entering SQL coercion rules.
    pub fn key(self, value: &Value) -> Result<Option<i64>, InspectError> {
        Ok(Some(match (self, value) {
            (_, Value::Null) => return Ok(None),
            (Self::Byte, Value::Byte(value)) => i64::from(*value),
            (Self::Int16, Value::Int16(value)) => i64::from(*value),
            (Self::Int32, Value::Int32(value)) => i64::from(*value),
            (Self::Int64, Value::Int64(value)) => *value,
            (Self::Currency, Value::Currency(value)) => value.scaled(),
            _ => return Err(InspectError::Malformed),
        }))
    }
}

pub(super) fn exact_column(
    index: &NativeIndex,
    schema: &TableSchema,
) -> Result<(usize, KeyType), InspectError> {
    if index.columns.len() != 1 || index.flags & IGNORE_NULLS != 0 || index.constraints().is_none()
    {
        return Err(InspectError::Unsupported);
    }
    let column = schema
        .columns
        .iter()
        .position(|c| c.number() == index.columns[0].number())
        .ok_or(InspectError::Malformed)?;
    let width = match schema.columns[column].scalar_type() {
        Some(ScalarType::Byte) => KeyType::Byte,
        Some(ScalarType::Int16) => KeyType::Int16,
        Some(ScalarType::Int32) => KeyType::Int32,
        Some(ScalarType::Int64) => KeyType::Int64,
        Some(ScalarType::Currency) => KeyType::Currency,
        _ => return Err(InspectError::Unsupported),
    };
    Ok((column, width))
}

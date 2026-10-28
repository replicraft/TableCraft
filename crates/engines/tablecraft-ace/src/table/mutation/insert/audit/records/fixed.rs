//! Exact fixed carriers, Boolean value masks and independently ordered GUID bytes.
use crate::{
    EditError, NativeColumn, WriteBlocker,
    format::data_type::{
        BOOLEAN, BYTE, CURRENCY, DATE_TIME, DOUBLE, GUID, INT16, INT32, INT64, SINGLE,
    },
};
use tablecraft_core::values::{Guid, Value};
use tablecraft_schema::ScalarType;

pub(super) fn width(field: &NativeColumn) -> Option<usize> {
    match (field.native_type(), field.scalar_type()) {
        (BOOLEAN, Some(ScalarType::Boolean)) => Some(0),
        (BYTE, Some(ScalarType::Byte)) => Some(size_of::<u8>()),
        (INT16, Some(ScalarType::Int16)) => Some(size_of::<i16>()),
        (INT32, Some(ScalarType::Int32)) => Some(size_of::<i32>()),
        (INT64, Some(ScalarType::Int64)) => Some(size_of::<i64>()),
        (CURRENCY, Some(ScalarType::Currency)) => Some(size_of::<i64>()),
        (SINGLE, Some(ScalarType::Single)) => Some(size_of::<f32>()),
        (DOUBLE, Some(ScalarType::Double)) => Some(size_of::<f64>()),
        (DATE_TIME, Some(ScalarType::DateTime)) => Some(size_of::<f64>()),
        (GUID, Some(ScalarType::Guid)) => Some(Guid::BYTE_LEN),
        _ => None,
    }
}

pub(super) fn write(
    field: &NativeColumn,
    value: &Value,
    target: &mut [u8],
) -> Result<bool, EditError> {
    if field.native_type() == BOOLEAN {
        return match value {
            Value::Boolean(value) if target.is_empty() => Ok(*value),
            _ => Err(EditError::Unsupported(WriteBlocker::Column)),
        };
    }

    if matches!(value, Value::Null) {
        target.fill(0);
        return Ok(false);
    }

    match (field.native_type(), value) {
        (BYTE, Value::Byte(value)) => put(target, &[*value])?,
        (INT16, Value::Int16(value)) => put(target, &value.to_le_bytes())?,
        (INT32, Value::Int32(value)) => put(target, &value.to_le_bytes())?,
        (INT64, Value::Int64(value)) => put(target, &value.to_le_bytes())?,
        (CURRENCY, Value::Currency(value)) => put(target, &value.scaled().to_le_bytes())?,
        (SINGLE, Value::Single(value)) => put(target, &value.to_bits().to_le_bytes())?,
        (DOUBLE, Value::Double(value)) => put(target, &value.to_bits().to_le_bytes())?,
        (DATE_TIME, Value::DateTime(value)) => put(target, &value.to_bits().to_le_bytes())?,
        (GUID, Value::Guid(value)) => {
            let mut wire = value.to_bytes();
            wire[..size_of::<u32>()].reverse();
            wire[size_of::<u32>()..size_of::<u32>() + size_of::<u16>()].reverse();
            wire[size_of::<u32>() + size_of::<u16>()..size_of::<u32>() + 2 * size_of::<u16>()]
                .reverse();
            put(target, &wire)?;
        }
        _ => return Err(EditError::Unsupported(WriteBlocker::Column)),
    }

    Ok(true)
}

fn put(target: &mut [u8], source: &[u8]) -> Result<(), EditError> {
    if target.len() != source.len() {
        return Err(EditError::Preservation);
    }
    target.copy_from_slice(source);
    Ok(())
}

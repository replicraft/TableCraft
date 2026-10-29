//! Exact fixed carriers and Boolean mask semantics for new ordinary records.
use crate::{
    EditError, NativeColumn, WriteBlocker,
    format::{
        data_type::EXTENDED_DATE_TIME_BYTES,
        layout::{FIXED_FLAG, UPDATABLE_FLAG},
    },
    table::{extended_datetime, mutation::fixed::encode_fixed},
};
use tablecraft_core::values::{Guid, Value};
use tablecraft_schema::ScalarType;

const BYTE_WIDTH: usize = size_of::<u8>();
const SHORT_WIDTH: usize = size_of::<i16>();
const LONG_WIDTH: usize = size_of::<i32>();
const WIDE_WIDTH: usize = size_of::<i64>();

pub(super) fn width(column: &NativeColumn) -> Result<usize, EditError> {
    let length = match column.scalar_type() {
        Some(ScalarType::Boolean) => 0,
        Some(ScalarType::Byte) => size_of::<u8>(),
        Some(ScalarType::Int16) => size_of::<i16>(),
        Some(ScalarType::Int32 | ScalarType::Single) => size_of::<i32>(),
        Some(
            ScalarType::Int64 | ScalarType::Currency | ScalarType::Double | ScalarType::DateTime,
        ) => size_of::<i64>(),
        Some(ScalarType::Guid) => Guid::BYTE_LEN,
        Some(ScalarType::DateTimeExtended) => EXTENDED_DATE_TIME_BYTES,
        _ => return Err(EditError::Unsupported(WriteBlocker::Column)),
    };
    if !column.fixed()
        || column.storage_length() != length
        || column.flags() != (FIXED_FLAG | UPDATABLE_FLAG)
        || column.extended_flags() != 0
        || column.complex_type_id().is_some()
    {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }
    Ok(length)
}

/// Return the native presence/value bit. Boolean has no nullable payload cell.
pub(super) fn write(
    column: &NativeColumn,
    value: &Value,
    destination: &mut [u8],
) -> Result<bool, EditError> {
    let kind = column
        .scalar_type()
        .ok_or(EditError::Unsupported(WriteBlocker::Column))?;
    if kind == ScalarType::Boolean {
        return match value {
            Value::Boolean(value) if destination.is_empty() => Ok(*value),
            _ => Err(EditError::Unsupported(WriteBlocker::Column)),
        };
    }
    if matches!(value, Value::Null) {
        destination.fill(0);
        return Ok(false);
    }
    if kind == ScalarType::DateTimeExtended {
        let Value::DateTimeExtended(value) = value else {
            return Err(EditError::Unsupported(WriteBlocker::Column));
        };
        if destination.len() != EXTENDED_DATE_TIME_BYTES {
            return Err(EditError::Unsupported(WriteBlocker::Column));
        }
        destination.copy_from_slice(&extended_datetime::encode(value)?);
        return Ok(true);
    }
    match destination.len() {
        BYTE_WIDTH => {
            destination.copy_from_slice(&encode_fixed::<{ size_of::<u8>() }>(kind, value)?)
        }
        SHORT_WIDTH => {
            destination.copy_from_slice(&encode_fixed::<{ size_of::<i16>() }>(kind, value)?)
        }
        LONG_WIDTH => {
            destination.copy_from_slice(&encode_fixed::<{ size_of::<i32>() }>(kind, value)?)
        }
        WIDE_WIDTH => {
            destination.copy_from_slice(&encode_fixed::<{ size_of::<i64>() }>(kind, value)?)
        }
        Guid::BYTE_LEN => {
            destination.copy_from_slice(&encode_fixed::<{ Guid::BYTE_LEN }>(kind, value)?)
        }
        _ => return Err(EditError::Unsupported(WriteBlocker::Column)),
    }
    Ok(true)
}

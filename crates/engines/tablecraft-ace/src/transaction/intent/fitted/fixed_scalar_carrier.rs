//! Closed TCNI21 exact scalar carriers; older fitted grammars remain separate.
use super::{
    carrier::{self, Borrowed, Encoded, Payload},
    model::Family,
};
use crate::transaction::intent::NativeIntentError;
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Double, LegacyDateTime, Single, Value},
};

const NULL: u8 = 0;
const INT64: u8 = 3;
const DATE: u8 = 5;
const BYTE: u8 = 8;
const INT16: u8 = 9;
const SINGLE: u8 = 10;
const DOUBLE: u8 = 11;

pub(super) fn view(value: &Value, family: Family) -> Result<Encoded<'_>, NativeIntentError> {
    if family != Family::Fixed {
        return Err(NativeIntentError::InvalidRecord);
    }
    let (tag, bytes, length) = match value {
        Value::Null => (NULL, [0; 8], 0),
        Value::Byte(value) => (BYTE, u64::from(*value).to_le_bytes(), 1),
        Value::Int16(value) => (INT16, u64::from(*value as u16).to_le_bytes(), 2),
        Value::Int64(value) => (INT64, value.to_le_bytes(), 8),
        Value::Single(value) if value.to_float().is_finite() => {
            (SINGLE, u64::from(value.to_bits()).to_le_bytes(), 4)
        }
        Value::Double(value) if value.to_float().is_finite() => {
            (DOUBLE, value.to_bits().to_le_bytes(), 8)
        }
        Value::DateTime(value) => (DATE, value.to_bits().to_le_bytes(), 8),
        _ => return Err(NativeIntentError::InvalidRecord),
    };
    Ok(Encoded {
        tag,
        payload: Payload::Fixed { bytes, length },
    })
}

pub(super) fn parse<'a>(
    tag: u8,
    bytes: &'a [u8],
    family: Family,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Borrowed<'a>, NativeIntentError> {
    if family != Family::Fixed {
        return Err(NativeIntentError::InvalidRecord);
    }
    let expected = match tag {
        NULL => 0,
        BYTE => 1,
        INT16 => 2,
        INT64 | DATE | DOUBLE => 8,
        SINGLE => 4,
        _ => return Err(NativeIntentError::InvalidRecord),
    };
    if bytes.len() != expected {
        return Err(NativeIntentError::InvalidRecord);
    }
    carrier::pay_scan(bytes, budget, cancelled)?;
    let value = match tag {
        NULL => Value::Null,
        BYTE => Value::Byte(bytes[0]),
        INT16 => Value::Int16(i16::from_le_bytes(
            bytes
                .try_into()
                .map_err(|_| NativeIntentError::InvalidRecord)?,
        )),
        INT64 => Value::Int64(i64::from_le_bytes(
            bytes
                .try_into()
                .map_err(|_| NativeIntentError::InvalidRecord)?,
        )),
        SINGLE => Value::Single(Single::from_bits(u32::from_le_bytes(
            bytes
                .try_into()
                .map_err(|_| NativeIntentError::InvalidRecord)?,
        ))),
        DOUBLE => Value::Double(Double::from_bits(u64::from_le_bytes(
            bytes
                .try_into()
                .map_err(|_| NativeIntentError::InvalidRecord)?,
        ))),
        DATE => Value::DateTime(
            LegacyDateTime::from_bits(u64::from_le_bytes(
                bytes
                    .try_into()
                    .map_err(|_| NativeIntentError::InvalidRecord)?,
            ))
            .map_err(|_| NativeIntentError::InvalidRecord)?,
        ),
        _ => return Err(NativeIntentError::InvalidRecord),
    };
    view(&value, family)?;
    Ok(Borrowed::Scalar(value))
}

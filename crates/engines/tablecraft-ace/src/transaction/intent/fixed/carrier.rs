use crate::transaction::intent::NativeIntentError;
use tablecraft_core::values::{Currency, LegacyDateTime, Value};

const TAG_BYTES: usize = size_of::<u8>();
const PAYLOAD_AT: usize = TAG_BYTES;
const PAYLOAD_BYTES: usize = size_of::<u64>();
const INT32_BYTES: usize = size_of::<i32>();
const TAG_AT: usize = 0;
const NULL_TAG: u8 = 0;
const BOOLEAN_TAG: u8 = 1;
const INT32_TAG: u8 = 2;
const INT64_TAG: u8 = 3;
const CURRENCY_TAG: u8 = 4;
const DATE_TAG: u8 = 5;
pub(super) const WIDTH: usize = TAG_BYTES + PAYLOAD_BYTES;

pub(super) fn encode(value: &Value) -> Result<[u8; WIDTH], NativeIntentError> {
    let mut result = [0; WIDTH];
    result[TAG_AT] = NULL_TAG;
    let payload = match value {
        Value::Null => return Ok(result),
        Value::Boolean(value) => {
            result[TAG_AT] = BOOLEAN_TAG;
            u64::from(*value).to_le_bytes()
        }
        Value::Int32(value) => {
            result[TAG_AT] = INT32_TAG;
            u64::from(*value as u32).to_le_bytes()
        }
        Value::Int64(value) => {
            result[TAG_AT] = INT64_TAG;
            value.to_le_bytes()
        }
        Value::Currency(value) => {
            result[TAG_AT] = CURRENCY_TAG;
            value.scaled().to_le_bytes()
        }
        Value::DateTime(value) => {
            result[TAG_AT] = DATE_TAG;
            value.to_bits().to_le_bytes()
        }
        _ => return Err(NativeIntentError::InvalidRecord),
    };
    result[PAYLOAD_AT..].copy_from_slice(&payload);
    Ok(result)
}

pub(super) fn decode(raw: &[u8; WIDTH]) -> Result<Value, NativeIntentError> {
    let payload: [u8; PAYLOAD_BYTES] = raw[PAYLOAD_AT..].try_into().unwrap();
    let value = match raw[TAG_AT] {
        NULL_TAG => Value::Null,
        BOOLEAN_TAG if payload[0] <= 1 => Value::Boolean(payload[0] != 0),
        INT32_TAG => Value::Int32(i32::from_le_bytes(
            payload[..INT32_BYTES].try_into().unwrap(),
        )),
        INT64_TAG => Value::Int64(i64::from_le_bytes(payload)),
        CURRENCY_TAG => Value::Currency(Currency::from_scaled(i64::from_le_bytes(payload))),
        DATE_TAG => Value::DateTime(
            LegacyDateTime::from_bits(u64::from_le_bytes(payload))
                .map_err(|_| NativeIntentError::InvalidRecord)?,
        ),
        _ => return Err(NativeIntentError::InvalidRecord),
    };
    if encode(&value)? != *raw {
        return Err(NativeIntentError::InvalidRecord);
    }
    Ok(value)
}

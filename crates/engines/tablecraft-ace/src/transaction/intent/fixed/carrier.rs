use crate::transaction::intent::NativeIntentError;
use tablecraft_core::values::{Currency, Double, Guid, LegacyDateTime, Single, Value};

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
const SINGLE_TAG: u8 = 6;
const DOUBLE_TAG: u8 = 7;
const BYTE_TAG: u8 = 8;
const INT16_TAG: u8 = 9;
const GUID_TAG: u8 = 10;
pub(super) const WIDTH: usize = TAG_BYTES + PAYLOAD_BYTES;
pub(super) const GUID_WIDTH: usize = TAG_BYTES + Guid::BYTE_LEN;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Carriers {
    Legacy,
    Ieee,
    Scalar,
    Guid,
}

impl Carriers {
    pub(super) const fn width(self) -> usize {
        if matches!(self, Self::Guid) {
            GUID_WIDTH
        } else {
            WIDTH
        }
    }

    pub(super) fn validate(self, value: &Value) -> Result<(), NativeIntentError> {
        if matches!(self, Self::Guid) {
            encode_guid(value)?;
        } else {
            self.encode(value)?;
        }
        Ok(())
    }

    pub(super) fn decode_cell(self, raw: &[u8]) -> Result<Value, NativeIntentError> {
        if matches!(self, Self::Guid) {
            decode_guid(
                raw.try_into()
                    .map_err(|_| NativeIntentError::InvalidRecord)?,
            )
        } else {
            self.decode(
                raw.try_into()
                    .map_err(|_| NativeIntentError::InvalidRecord)?,
            )
        }
    }

    pub(super) fn encode(self, value: &Value) -> Result<[u8; WIDTH], NativeIntentError> {
        match self {
            Self::Legacy => encode(value),
            Self::Ieee => encode_ieee(value),
            Self::Scalar => encode_scalar(value),
            Self::Guid => Err(NativeIntentError::InvalidRecord),
        }
    }

    pub(super) fn decode(self, raw: &[u8; WIDTH]) -> Result<Value, NativeIntentError> {
        match self {
            Self::Legacy => decode(raw),
            Self::Ieee => decode_ieee(raw),
            Self::Scalar => decode_scalar(raw),
            Self::Guid => Err(NativeIntentError::InvalidRecord),
        }
    }
}

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

/// Extended ordinary records retain the legacy cells byte-for-byte.
/// These additional carriers are request data, never source permission.
pub(super) fn encode_ieee(value: &Value) -> Result<[u8; WIDTH], NativeIntentError> {
    let mut raw = [0; WIDTH];
    match value {
        Value::Single(value) => {
            raw[TAG_AT] = SINGLE_TAG;
            raw[PAYLOAD_AT..PAYLOAD_AT + size_of::<u32>()]
                .copy_from_slice(&value.to_bits().to_le_bytes());
        }
        Value::Double(value) => {
            raw[TAG_AT] = DOUBLE_TAG;
            raw[PAYLOAD_AT..].copy_from_slice(&value.to_bits().to_le_bytes());
        }
        _ => return encode(value),
    }
    Ok(raw)
}

pub(super) fn decode_ieee(raw: &[u8; WIDTH]) -> Result<Value, NativeIntentError> {
    let value = match raw[TAG_AT] {
        SINGLE_TAG => Value::Single(Single::from_bits(u32::from_le_bytes(
            raw[PAYLOAD_AT..PAYLOAD_AT + size_of::<u32>()]
                .try_into()
                .unwrap(),
        ))),
        DOUBLE_TAG => Value::Double(Double::from_bits(u64::from_le_bytes(
            raw[PAYLOAD_AT..].try_into().unwrap(),
        ))),
        _ => return decode(raw),
    };
    if encode_ieee(&value)? != *raw {
        return Err(NativeIntentError::InvalidRecord);
    }
    Ok(value)
}

/// Cumulative original cells retain every legacy/IEEE representation unchanged.
pub(super) fn encode_scalar(value: &Value) -> Result<[u8; WIDTH], NativeIntentError> {
    let mut raw = [0; WIDTH];
    match value {
        Value::Byte(value) => {
            raw[TAG_AT] = BYTE_TAG;
            raw[PAYLOAD_AT] = *value;
        }
        Value::Int16(value) => {
            raw[TAG_AT] = INT16_TAG;
            raw[PAYLOAD_AT..PAYLOAD_AT + size_of::<i16>()].copy_from_slice(&value.to_le_bytes());
        }
        _ => return encode_ieee(value),
    }
    Ok(raw)
}

pub(super) fn decode_scalar(raw: &[u8; WIDTH]) -> Result<Value, NativeIntentError> {
    let value = match raw[TAG_AT] {
        BYTE_TAG => Value::Byte(raw[PAYLOAD_AT]),
        INT16_TAG => Value::Int16(i16::from_le_bytes(
            raw[PAYLOAD_AT..PAYLOAD_AT + size_of::<i16>()]
                .try_into()
                .unwrap(),
        )),
        _ => return decode_ieee(raw),
    };
    if encode_scalar(&value)? != *raw {
        return Err(NativeIntentError::InvalidRecord);
    }
    Ok(value)
}

/// GUID request bytes are canonical, deliberately distinct from native DATA.
pub(super) fn encode_guid(value: &Value) -> Result<[u8; GUID_WIDTH], NativeIntentError> {
    let mut raw = [0; GUID_WIDTH];
    if let Value::Guid(value) = value {
        raw[TAG_AT] = GUID_TAG;
        raw[PAYLOAD_AT..].copy_from_slice(&value.to_bytes());
    } else {
        raw[..WIDTH].copy_from_slice(&encode_scalar(value)?);
    }
    Ok(raw)
}

pub(super) fn decode_guid(raw: &[u8; GUID_WIDTH]) -> Result<Value, NativeIntentError> {
    let value = if raw[TAG_AT] == GUID_TAG {
        Value::Guid(Guid::from_bytes(raw[PAYLOAD_AT..].try_into().unwrap()))
    } else {
        decode_scalar(raw[..WIDTH].try_into().unwrap())?
    };
    if encode_guid(&value)? != *raw {
        return Err(NativeIntentError::InvalidRecord);
    }
    Ok(value)
}

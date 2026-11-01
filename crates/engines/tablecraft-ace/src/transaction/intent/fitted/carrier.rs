//! Length-delimited original carriers; native field encodings are not stored.
use super::model::Family;
use crate::{InspectError, header::check, transaction::intent::NativeIntentError};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{Currency, LegacyDateTime, Value},
};

pub(super) const MAX_VARIABLE_BYTES: usize =
    crate::limits::MAX_ENCODED_TEXT_BYTES * char::MAX.len_utf8();
pub(super) const HEADER_BYTES: usize = 8;
pub(super) const COPY_BYTES: usize = 4096;
const NULL: u8 = 0;
const BOOLEAN: u8 = 1;
const INT32: u8 = 2;
const INT64: u8 = 3;
const CURRENCY: u8 = 4;
const DATE: u8 = 5;
const TEXT: u8 = 6;
const BINARY: u8 = 7;

pub(super) enum Payload<'a> {
    Fixed { bytes: [u8; 8], length: usize },
    Borrowed(&'a [u8]),
}

pub(super) struct Encoded<'a> {
    pub tag: u8,
    pub payload: Payload<'a>,
}

impl Encoded<'_> {
    pub fn bytes(&self) -> &[u8] {
        match &self.payload {
            Payload::Fixed { bytes, length } => &bytes[..*length],
            Payload::Borrowed(bytes) => bytes,
        }
    }
}

fn allowed(family: Family, tag: u8) -> bool {
    match family {
        Family::Fixed => matches!(tag, NULL | BOOLEAN | INT32 | INT64 | CURRENCY | DATE),
        Family::Variable => matches!(tag, NULL | TEXT | BINARY),
    }
}

pub(super) fn view(value: &Value, family: Family) -> Result<Encoded<'_>, NativeIntentError> {
    let (tag, payload) = match value {
        Value::Null => (NULL, Payload::Borrowed(&[])),
        Value::Boolean(value) => (
            BOOLEAN,
            Payload::Fixed {
                bytes: u64::from(*value).to_le_bytes(),
                length: 1,
            },
        ),
        Value::Int32(value) => (
            INT32,
            Payload::Fixed {
                bytes: u64::from(*value as u32).to_le_bytes(),
                length: 4,
            },
        ),
        Value::Int64(value) => (
            INT64,
            Payload::Fixed {
                bytes: value.to_le_bytes(),
                length: 8,
            },
        ),
        Value::Currency(value) => (
            CURRENCY,
            Payload::Fixed {
                bytes: value.scaled().to_le_bytes(),
                length: 8,
            },
        ),
        Value::DateTime(value) => (
            DATE,
            Payload::Fixed {
                bytes: value.to_bits().to_le_bytes(),
                length: 8,
            },
        ),
        Value::Text(value) => (TEXT, Payload::Borrowed(value.as_bytes())),
        Value::Binary(value) => (BINARY, Payload::Borrowed(value)),
        _ => return Err(NativeIntentError::InvalidRecord),
    };
    let result = Encoded { tag, payload };
    if !allowed(family, tag) || result.bytes().len() > MAX_VARIABLE_BYTES {
        return Err(NativeIntentError::InvalidRecord);
    }
    Ok(result)
}

pub(super) fn pay_scan(
    bytes: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), NativeIntentError> {
    for chunk in bytes.chunks(COPY_BYTES) {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, chunk.len() as u64)
            .map_err(InspectError::from)?;
    }
    check(budget, cancelled)?;
    Ok(())
}

pub(super) fn validate(
    value: &Value,
    family: Family,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), NativeIntentError> {
    let encoded = view(value, family)?;
    pay_scan(encoded.bytes(), budget, cancelled)
}

pub(super) enum Borrowed<'a> {
    Scalar(Value),
    Text(&'a str),
    Binary(&'a [u8]),
}

pub(super) fn parse<'a>(
    tag: u8,
    bytes: &'a [u8],
    family: Family,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Borrowed<'a>, NativeIntentError> {
    if !allowed(family, tag) || bytes.len() > MAX_VARIABLE_BYTES {
        return Err(NativeIntentError::InvalidRecord);
    }
    pay_scan(bytes, budget, cancelled)?;
    let scalar = match tag {
        NULL if bytes.is_empty() => Value::Null,
        BOOLEAN if bytes.len() == 1 && bytes[0] <= 1 => Value::Boolean(bytes[0] != 0),
        INT32 if bytes.len() == 4 => Value::Int32(i32::from_le_bytes(bytes.try_into().unwrap())),
        INT64 if bytes.len() == 8 => Value::Int64(i64::from_le_bytes(bytes.try_into().unwrap())),
        CURRENCY if bytes.len() == 8 => Value::Currency(Currency::from_scaled(i64::from_le_bytes(
            bytes.try_into().unwrap(),
        ))),
        DATE if bytes.len() == 8 => Value::DateTime(
            LegacyDateTime::from_bits(u64::from_le_bytes(bytes.try_into().unwrap()))
                .map_err(|_| NativeIntentError::InvalidRecord)?,
        ),
        TEXT => {
            return Ok(Borrowed::Text(
                std::str::from_utf8(bytes).map_err(|_| NativeIntentError::InvalidRecord)?,
            ));
        }
        BINARY => return Ok(Borrowed::Binary(bytes)),
        _ => return Err(NativeIntentError::InvalidRecord),
    };
    Ok(Borrowed::Scalar(scalar))
}

pub(super) fn owned(
    value: Borrowed<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Value, NativeIntentError> {
    match value {
        Borrowed::Scalar(value) => Ok(value),
        Borrowed::Text(value) => {
            budget
                .charge_allocation(value.len() as u64, 1)
                .map_err(InspectError::from)?;
            let mut result = String::new();
            result
                .try_reserve_exact(value.len())
                .map_err(|_| InspectError::Allocation)?;
            // Chunk at UTF8 boundaries; each copy has paid work and cancellation.
            let mut at = 0;
            while at < value.len() {
                check(budget, cancelled)?;
                let mut end = value.len().min(at + COPY_BYTES);
                while !value.is_char_boundary(end) {
                    end -= 1;
                }
                budget
                    .charge(Resource::Steps, (end - at) as u64)
                    .map_err(InspectError::from)?;
                result.push_str(&value[at..end]);
                at = end;
            }
            check(budget, cancelled)?;
            Ok(Value::Text(result))
        }
        Borrowed::Binary(value) => {
            budget
                .charge_allocation(value.len() as u64, 1)
                .map_err(InspectError::from)?;
            let mut result = Vec::new();
            result
                .try_reserve_exact(value.len())
                .map_err(|_| InspectError::Allocation)?;
            for chunk in value.chunks(COPY_BYTES) {
                check(budget, cancelled)?;
                budget
                    .charge(Resource::Steps, chunk.len() as u64)
                    .map_err(InspectError::from)?;
                result.extend_from_slice(chunk);
            }
            check(budget, cancelled)?;
            Ok(Value::Binary(result))
        }
    }
}

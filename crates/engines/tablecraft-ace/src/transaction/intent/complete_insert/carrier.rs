//! TCNA4-only exact request carriers; legacy protocols retain their old grammar.
use super::{NativeIntentError, Protocol};
use crate::transaction::intent::fitted::carrier as legacy;
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Decimal, Double, Guid, Single, Value},
};

pub(super) use legacy::{Borrowed, COPY_BYTES, HEADER_BYTES, owned, pay_scan};

const BYTE: u8 = 8;
const INT16: u8 = 9;
const SINGLE: u8 = 10;
const DOUBLE: u8 = 11;
const GUID: u8 = 12;
const DECIMAL: u8 = 13;
const DECIMAL_BYTES: usize = size_of::<i128>() + size_of::<u8>();

pub(super) enum Encoded<'a> {
    Legacy(legacy::Encoded<'a>),
    Fixed {
        tag: u8,
        bytes: [u8; DECIMAL_BYTES],
        length: usize,
    },
}

impl Encoded<'_> {
    pub(super) fn tag(&self) -> u8 {
        match self {
            Self::Legacy(value) => value.tag,
            Self::Fixed { tag, .. } => *tag,
        }
    }

    pub(super) fn bytes(&self) -> &[u8] {
        match self {
            Self::Legacy(value) => value.bytes(),
            Self::Fixed { bytes, length, .. } => &bytes[..*length],
        }
    }
}

fn fixed(tag: u8, value: &[u8]) -> Encoded<'static> {
    let mut bytes = [0; DECIMAL_BYTES];
    bytes[..value.len()].copy_from_slice(value);
    Encoded::Fixed {
        tag,
        bytes,
        length: value.len(),
    }
}

pub(super) fn row_view(
    value: &Value,
    protocol: Protocol,
) -> Result<Encoded<'_>, NativeIntentError> {
    if !matches!(protocol, Protocol::Complete) {
        return legacy::row_view(value).map(Encoded::Legacy);
    }
    let result = match value {
        Value::Byte(value) => fixed(BYTE, &[*value]),
        Value::Int16(value) => fixed(INT16, &value.to_le_bytes()),
        Value::Single(value) => fixed(SINGLE, &value.to_bits().to_le_bytes()),
        Value::Double(value) => fixed(DOUBLE, &value.to_bits().to_le_bytes()),
        Value::Guid(value) => fixed(GUID, &value.to_bytes()),
        Value::Decimal(value) => {
            let mut bytes = [0; DECIMAL_BYTES];
            bytes[..size_of::<i128>()].copy_from_slice(&value.coefficient().to_le_bytes());
            bytes[size_of::<i128>()] = value.scale();
            Encoded::Fixed {
                tag: DECIMAL,
                bytes,
                length: DECIMAL_BYTES,
            }
        }
        _ => return legacy::row_view(value).map(Encoded::Legacy),
    };
    Ok(result)
}

pub(super) fn row_parse<'a>(
    tag: u8,
    bytes: &'a [u8],
    protocol: Protocol,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Borrowed<'a>, NativeIntentError> {
    if tag < BYTE || !matches!(protocol, Protocol::Complete) {
        return legacy::row_parse(tag, bytes, budget, cancelled);
    }
    pay_scan(bytes, budget, cancelled)?;
    let scalar = match tag {
        BYTE if bytes.len() == size_of::<u8>() => Value::Byte(bytes[0]),
        INT16 if bytes.len() == size_of::<i16>() => {
            Value::Int16(i16::from_le_bytes(bytes.try_into().unwrap()))
        }
        SINGLE if bytes.len() == size_of::<u32>() => Value::Single(Single::from_bits(
            u32::from_le_bytes(bytes.try_into().unwrap()),
        )),
        DOUBLE if bytes.len() == size_of::<u64>() => Value::Double(Double::from_bits(
            u64::from_le_bytes(bytes.try_into().unwrap()),
        )),
        GUID if bytes.len() == Guid::BYTE_LEN => {
            Value::Guid(Guid::from_bytes(bytes.try_into().unwrap()))
        }
        DECIMAL if bytes.len() == DECIMAL_BYTES => {
            let coefficient = i128::from_le_bytes(bytes[..size_of::<i128>()].try_into().unwrap());
            let value = Decimal::new(coefficient, bytes[size_of::<i128>()])
                .map_err(|_| NativeIntentError::InvalidRecord)?;
            Value::Decimal(value)
        }
        _ => return Err(NativeIntentError::InvalidRecord),
    };
    Ok(Borrowed::Scalar(scalar))
}

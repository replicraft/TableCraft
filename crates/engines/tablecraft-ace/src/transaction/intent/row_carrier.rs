//! Exact row/default carriers selected by closed protocol grammars.
use super::NativeIntentError;
use crate::transaction::intent::fitted::carrier as legacy;
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Decimal, Double, Guid, Single, Value},
};

pub(super) use legacy::{Borrowed, COPY_BYTES, HEADER_BYTES, owned, pay_scan};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Profile {
    Legacy,
    Wider,
}

impl Profile {
    pub(super) const fn for_value(value: &Value) -> Self {
        match value {
            Value::Byte(_)
            | Value::Int16(_)
            | Value::Single(_)
            | Value::Double(_)
            | Value::Guid(_)
            | Value::Decimal(_) => Self::Wider,
            _ => Self::Legacy,
        }
    }
}

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

#[derive(Clone, Copy)]
enum VariablePolicy {
    Fitted,
    WireLength,
}

impl VariablePolicy {
    fn view(self, value: &Value) -> Result<Encoded<'_>, NativeIntentError> {
        match self {
            Self::Fitted => legacy::row_view(value),
            Self::WireLength => legacy::length_prefixed_row_view(value),
        }
        .map(Encoded::Legacy)
    }

    fn parse<'a>(
        self,
        tag: u8,
        bytes: &'a [u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Borrowed<'a>, NativeIntentError> {
        match self {
            Self::Fitted => legacy::row_parse(tag, bytes, budget, cancelled),
            Self::WireLength => legacy::length_prefixed_row_parse(tag, bytes, budget, cancelled),
        }
    }
}

pub(super) fn row_view(value: &Value, profile: Profile) -> Result<Encoded<'_>, NativeIntentError> {
    view_with_policy(value, profile, VariablePolicy::Fitted)
}

pub(super) fn length_prefixed_row_view(
    value: &Value,
    profile: Profile,
) -> Result<Encoded<'_>, NativeIntentError> {
    view_with_policy(value, profile, VariablePolicy::WireLength)
}

fn view_with_policy(
    value: &Value,
    profile: Profile,
    variables: VariablePolicy,
) -> Result<Encoded<'_>, NativeIntentError> {
    if !matches!(profile, Profile::Wider) {
        return variables.view(value);
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
        _ => return variables.view(value),
    };
    Ok(result)
}

pub(super) fn row_parse<'a>(
    tag: u8,
    bytes: &'a [u8],
    profile: Profile,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Borrowed<'a>, NativeIntentError> {
    parse_with_policy(
        tag,
        bytes,
        profile,
        VariablePolicy::Fitted,
        budget,
        cancelled,
    )
}

pub(super) fn length_prefixed_row_parse<'a>(
    tag: u8,
    bytes: &'a [u8],
    profile: Profile,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Borrowed<'a>, NativeIntentError> {
    parse_with_policy(
        tag,
        bytes,
        profile,
        VariablePolicy::WireLength,
        budget,
        cancelled,
    )
}

fn parse_with_policy<'a>(
    tag: u8,
    bytes: &'a [u8],
    profile: Profile,
    variables: VariablePolicy,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Borrowed<'a>, NativeIntentError> {
    if tag < BYTE || !matches!(profile, Profile::Wider) {
        return variables.parse(tag, bytes, budget, cancelled);
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

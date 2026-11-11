//! TCNI22 original GUID and Decimal values, separate from native field layouts.
use super::{
    carrier::{self, Borrowed, Encoded, Payload},
    model::Family,
};
use crate::transaction::intent::NativeIntentError;
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Decimal, Guid, Value},
};

const NULL: u8 = 0;
const GUID: u8 = 12;
const DECIMAL: u8 = 13;
const GUID_BYTES: usize = size_of::<[u8; 16]>();
const DECIMAL_BYTES: usize = size_of::<i128>() + size_of::<u8>();

pub(super) fn view(value: &Value, family: Family) -> Result<Encoded<'_>, NativeIntentError> {
    let mut bytes = [0; DECIMAL_BYTES];
    let (tag, length) = match (family, value) {
        (_, Value::Null) => (NULL, 0),
        (Family::Fixed, Value::Guid(value)) => {
            bytes[..GUID_BYTES].copy_from_slice(&value.to_bytes());
            (GUID, GUID_BYTES)
        }
        (Family::Variable, Value::Decimal(value)) => {
            bytes[..size_of::<i128>()].copy_from_slice(&value.coefficient().to_le_bytes());
            bytes[size_of::<i128>()] = value.scale();
            (DECIMAL, DECIMAL_BYTES)
        }
        _ => return Err(NativeIntentError::InvalidRecord),
    };
    Ok(Encoded {
        tag,
        payload: Payload::Wide { bytes, length },
    })
}

pub(super) fn parse<'a>(
    tag: u8,
    bytes: &'a [u8],
    family: Family,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Borrowed<'a>, NativeIntentError> {
    let length = match (family, tag) {
        (_, NULL) => 0,
        (Family::Fixed, GUID) => GUID_BYTES,
        (Family::Variable, DECIMAL) => DECIMAL_BYTES,
        _ => return Err(NativeIntentError::InvalidRecord),
    };
    if bytes.len() != length {
        return Err(NativeIntentError::InvalidRecord);
    }
    carrier::pay_scan(bytes, budget, cancelled)?;
    let value = match tag {
        NULL => Value::Null,
        GUID => Value::Guid(Guid::from_bytes(
            bytes
                .try_into()
                .map_err(|_| NativeIntentError::InvalidRecord)?,
        )),
        DECIMAL => {
            let coefficient = i128::from_le_bytes(
                bytes[..size_of::<i128>()]
                    .try_into()
                    .map_err(|_| NativeIntentError::InvalidRecord)?,
            );
            Value::Decimal(
                Decimal::new(coefficient, bytes[size_of::<i128>()])
                    .map_err(|_| NativeIntentError::InvalidRecord)?,
            )
        }
        _ => return Err(NativeIntentError::InvalidRecord),
    };
    Ok(Borrowed::Scalar(value))
}

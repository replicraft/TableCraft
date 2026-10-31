//! Retain legacy Decimal admission while sharing the other exact fixed codecs.
use super::super::{EditError, WriteBlocker};
use crate::table::scalar_encoding;
use tablecraft_core::values::Value;
use tablecraft_schema::ScalarType;

pub(in crate::table::mutation) fn encode<const WIDTH: usize>(
    kind: ScalarType,
    value: &Value,
) -> Result<[u8; WIDTH], EditError> {
    match (kind, value) {
        (ScalarType::Decimal, Value::Decimal(value)) => {
            bytes(crate::table::decimal::encode(*value))
        }
        _ => {
            scalar_encoding::encode(kind, value).ok_or(EditError::Unsupported(WriteBlocker::Column))
        }
    }
}

fn bytes<const WIDTH: usize, const SOURCE: usize>(
    value: [u8; SOURCE],
) -> Result<[u8; WIDTH], EditError> {
    value
        .as_slice()
        .try_into()
        .map_err(|_| EditError::Unsupported(WriteBlocker::Column))
}

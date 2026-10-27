//! Independently derive exact payload authority from immutable request text.
use super::super::super::EditError;
use super::super::{encode::Encoded, stage::Change};
use crate::{
    InspectError, NativeColumn,
    format::text,
    header::check,
    limits::{MAX_LONG_TEXT_BYTES, MAX_LONG_VALUE_BYTES},
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

const CHECK_UNITS: usize = 256;
const MEMO_COMPRESSION_LIMIT: usize = 1024;
const COMPRESSED_MEMO_EXTENDED_FLAGS: u8 = 1;
const MARKER: [u8; 2] = [0xff, 0xfe];

#[cfg(test)]
mod tests;

pub(super) fn prove<'a>(
    field: &NativeColumn,
    change: &'a Change<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<&'a [u8], EditError> {
    let bytes = match &change.encoded {
        Encoded::Null => {
            if !matches!(change.root.update.value(), Value::Null) {
                return Err(EditError::Preservation);
            }

            return Ok(&[]);
        }
        Encoded::Present(bytes) => bytes.as_slice(),
    };
    budget
        .charge(Resource::Steps, bytes.len() as u64)
        .map_err(InspectError::from)?;

    match change.root.update.value() {
        Value::Binary(expected) => {
            if bytes.len() != expected.len() {
                return Err(EditError::Preservation);
            }
            for (actual, expected) in bytes.chunks(CHECK_UNITS).zip(expected.chunks(CHECK_UNITS)) {
                check(budget, cancelled)?;
                if actual != expected {
                    return Err(EditError::Preservation);
                }
            }
        }
        Value::Text(expected) => {
            budget
                .charge(Resource::Steps, expected.len() as u64)
                .map_err(InspectError::from)?;
            let mut count = 0_usize;
            let mut narrow = true;
            for unit in expected.encode_utf16() {
                if count.is_multiple_of(CHECK_UNITS) {
                    check(budget, cancelled)?;
                }
                count += 1;
                narrow &= unit != 0 && unit <= u16::from(u8::MAX);
            }

            // This verifier does not call the encoder or trust its chosen mode.
            let compressed = field.extended_flags() == COMPRESSED_MEMO_EXTENDED_FLAGS
                && (MARKER.len() + 1..=MEMO_COMPRESSION_LIMIT).contains(&count)
                && narrow;
            let length = if compressed {
                count.checked_add(MARKER.len())
            } else {
                count.checked_mul(size_of::<u16>())
            }
            .ok_or(EditError::Preservation)?;
            if bytes.len() != length || (compressed && !bytes.starts_with(&MARKER)) {
                return Err(EditError::Preservation);
            }

            budget
                .charge(Resource::Steps, expected.len() as u64)
                .map_err(InspectError::from)?;
            for (position, unit) in expected.encode_utf16().enumerate() {
                if position.is_multiple_of(CHECK_UNITS) {
                    check(budget, cancelled)?;
                }
                let equal = if compressed {
                    u16::from(bytes[MARKER.len() + position]) == unit
                } else {
                    let at = position * size_of::<u16>();
                    bytes[at..at + size_of::<u16>()] == unit.to_le_bytes()
                };
                if !equal {
                    return Err(EditError::Preservation);
                }
            }

            let decoded = text::decode_long_with_limits(
                bytes,
                MAX_LONG_VALUE_BYTES,
                MAX_LONG_TEXT_BYTES,
                budget,
                cancelled,
            )?;
            if decoded != *expected {
                return Err(EditError::Preservation);
            }
        }
        _ => return Err(EditError::Preservation),
    }

    Ok(bytes)
}

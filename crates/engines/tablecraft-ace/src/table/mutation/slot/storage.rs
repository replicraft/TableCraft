//! Replacement bytes for an existing slot; no offset directory is rewritten.
use super::super::{EditError, WriteBlocker};
use crate::{
    InspectError, NativeColumn,
    format::{
        data_type::{MAX_SHORT_BINARY_BYTES, MAX_SHORT_TEXT_UNITS, SHORT_BINARY, SHORT_TEXT},
        layout::{UNICODE_COMPRESSION_FLAG, UPDATABLE_FLAG},
        text,
    },
    header::check,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

const COMPRESSED_MARKER: [u8; 2] = [0xff, 0xfe];
const UTF16_BYTES: usize = size_of::<u16>();

pub(in crate::table::mutation) fn validate(
    column: &NativeColumn,
    value: &Value,
) -> Result<(), EditError> {
    let valid = match value {
        Value::Null => true,
        Value::Text(text) if column.native_type() == SHORT_TEXT => {
            text.encode_utf16()
                .take(column.storage_length() / UTF16_BYTES + 1)
                .count()
                <= column.storage_length() / UTF16_BYTES
        }
        Value::Binary(bytes) if column.native_type() == SHORT_BINARY => {
            bytes.len() <= column.storage_length()
        }
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(EditError::Unsupported(WriteBlocker::Column))
    }
}

pub(in crate::table::mutation) fn qualify(column: &NativeColumn) -> Result<(), EditError> {
    let supported = !column.fixed()
        && match column.native_type() {
            SHORT_TEXT => {
                column.field.flags == UPDATABLE_FLAG
                    && column.storage_length() / UTF16_BYTES <= MAX_SHORT_TEXT_UNITS
                    && column.field.extended_flags & !UNICODE_COMPRESSION_FLAG == 0
                    && column.storage_length().is_multiple_of(UTF16_BYTES)
            }
            SHORT_BINARY => {
                column.field.flags == UPDATABLE_FLAG
                    && column.storage_length() <= MAX_SHORT_BINARY_BYTES
                    && column.field.extended_flags == 0
            }
            _ => false,
        };
    if supported {
        Ok(())
    } else {
        Err(EditError::Unsupported(WriteBlocker::Column))
    }
}

/// Select a proven native representation which fits this existing slot. The
/// compression flag expresses the preferred form, never extra slot capacity.
pub(in crate::table::mutation) fn encode_for_span(
    column: &NativeColumn,
    value: &Value,
    span: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    validate(column, value)?;
    check(budget, cancelled)?;

    let compressed = match value {
        Value::Binary(bytes) if bytes.len() == span => false,
        Value::Text(text) => {
            let count = text.encode_utf16().count();
            let raw_fits = count * UTF16_BYTES == span;
            // The marker alone is an observed empty native representation.
            let compressed_fits = COMPRESSED_MARKER.len() + count == span
                && text
                    .encode_utf16()
                    .all(|unit| unit > 0 && unit <= u16::from(u8::MAX));
            if !raw_fits && !compressed_fits {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }
            compressed_fits
                && (!raw_fits || column.field.extended_flags & UNICODE_COMPRESSION_FLAG != 0)
        }
        _ => return Err(EditError::Unsupported(WriteBlocker::RowLayout)),
    };
    budget
        .charge_allocation(span as u64, 1)
        .map_err(InspectError::from)?;
    budget
        .charge(Resource::Steps, span as u64)
        .map_err(InspectError::from)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(span)
        .map_err(|_| InspectError::Allocation)?;

    match value {
        Value::Binary(value) => bytes.extend_from_slice(value),
        Value::Text(value) => {
            if compressed {
                bytes.extend_from_slice(&COMPRESSED_MARKER);
                for unit in value.encode_utf16() {
                    check(budget, cancelled)?;
                    bytes.push(unit as u8);
                }
            } else {
                for unit in value.encode_utf16() {
                    check(budget, cancelled)?;
                    bytes.extend_from_slice(&unit.to_le_bytes());
                }
            }
            // A leading U+FEFF in plain UTF-16 aliases the compression marker.
            // Prove the selected representation with the existing decoder;
            // never qualify a payload whose visible value changes on reopen.
            if text::decode(&bytes, true, budget)? != *value {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }
        }
        _ => return Err(EditError::Unsupported(WriteBlocker::Column)),
    }
    if bytes.len() != span {
        return Err(EditError::Preservation);
    }
    check(budget, cancelled)?;
    Ok(bytes)
}

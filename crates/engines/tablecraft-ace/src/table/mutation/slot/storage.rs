//! Qualified Short Text/Binary serialization for existing slots and complete rows.
//! Exact-span and canonical complete-row policies remain separate.
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

/// Prefer declared compression within the selected field's actual capacity.
/// Legal raw UTF16 remains available when the marker would exceed that capacity.
/// Ordinary variable updates continue to use the minimum-size row policy.
pub(in crate::table::mutation) fn encode_declared(
    field: &NativeColumn,
    value: &Value,
    capacity: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    validate(field, value)?;
    check(budget, cancelled)?;

    let Value::Text(value_text) = value else {
        let length = match value {
            Value::Null => 0,
            Value::Binary(bytes) => bytes.len(),
            _ => return Err(EditError::Unsupported(WriteBlocker::Column)),
        };
        if length > capacity {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        return encode_preferred(field, value, budget, cancelled);
    };
    let mut units = 0_usize;
    let mut narrow = true;
    for unit in value_text.encode_utf16() {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, 1)
            .map_err(InspectError::from)?;
        units = units.checked_add(1).ok_or(InspectError::StructureLimit)?;
        narrow &= unit != 0 && unit <= u16::from(u8::MAX);
    }
    let declared_length = COMPRESSED_MARKER
        .len()
        .checked_add(units)
        .ok_or(InspectError::StructureLimit)?;
    let raw_length = units
        .checked_mul(UTF16_BYTES)
        .ok_or(InspectError::StructureLimit)?;
    let length = if field.extended_flags() & UNICODE_COMPRESSION_FLAG != 0
        && narrow
        && declared_length <= capacity
    {
        declared_length
    } else if raw_length <= capacity {
        raw_length
    } else {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    };
    encode_for_span(field, value, length, budget, cancelled)
}

/// Serialize an already qualified field with the canonical complete-row policy.
/// This chooses payload bytes independently of any existing slot capacity; the
/// caller owns field admission and the surrounding record offsets and masks.
pub(in crate::table::mutation) fn encode_preferred(
    field: &NativeColumn,
    value: &Value,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    validate(field, value)?;
    check(budget, cancelled)?;
    let length = match value {
        Value::Null => return Ok(Vec::new()),
        Value::Binary(bytes) => bytes.len(),
        Value::Text(value) => {
            let count = value.encode_utf16().count();
            let compact = field.extended_flags() & UNICODE_COMPRESSION_FLAG != 0
                && count > COMPRESSED_MARKER.len()
                && value
                    .encode_utf16()
                    .all(|unit| unit > 0 && unit <= u16::from(u8::MAX));
            if compact {
                COMPRESSED_MARKER.len() + count
            } else {
                UTF16_BYTES * count
            }
        }
        _ => return Err(EditError::Unsupported(WriteBlocker::Column)),
    };
    // The exact-span chooser permits either valid form on equal size. Preferred
    // row serialization uses raw UTF16 for the two-unit compression tie, as the
    // independent public update observations establish. Never mask metadata to
    // borrow a different encoder profile.
    if field.native_type() == SHORT_TEXT
        && field.extended_flags() & UNICODE_COMPRESSION_FLAG != 0
        && let Value::Text(value) = value
        && value.encode_utf16().count() == COMPRESSED_MARKER.len()
    {
        budget
            .charge_allocation(length as u64, 1)
            .map_err(InspectError::from)?;
        budget
            .charge(Resource::Steps, length as u64)
            .map_err(InspectError::from)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(length)
            .map_err(|_| InspectError::Allocation)?;
        for unit in value.encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        if text::decode(&bytes, true, budget)? != *value {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        return Ok(bytes);
    }
    encode_for_span(field, value, length, budget, cancelled)
}

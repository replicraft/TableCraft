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
const WIDE_SWITCH: u8 = 0;
const ESCAPE_PREFIX: [u8; 3] = [0xff, 0xfe, WIDE_SWITCH];

pub(in crate::table::mutation) fn raw_text_length(
    text: &str,
    units: usize,
) -> Result<usize, EditError> {
    units
        .checked_mul(UTF16_BYTES)
        .and_then(|bytes| {
            bytes.checked_add(if text.starts_with('\u{feff}') {
                ESCAPE_PREFIX.len()
            } else {
                0
            })
        })
        .ok_or_else(|| InspectError::StructureLimit.into())
}

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

    if let Value::Text(value) = value {
        return encode_text_for_span(column, value, span, budget, cancelled);
    }
    let Value::Binary(value) = value else {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    };
    if value.len() != span {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }
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
    bytes.extend_from_slice(value);
    check(budget, cancelled)?;
    Ok(bytes)
}

fn encode_text_for_span(
    column: &NativeColumn,
    value: &str,
    span: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    let count = value.encode_utf16().count();
    let raw_fits = raw_text_length(value, count)? == span;
    let compressed_fits = COMPRESSED_MARKER.len() + count == span
        && value
            .encode_utf16()
            .all(|unit| unit > 0 && unit <= u16::from(u8::MAX));
    if !raw_fits && !compressed_fits {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    let compressed = compressed_fits
        && (!raw_fits || column.field.extended_flags & UNICODE_COMPRESSION_FLAG != 0);
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
    if compressed {
        bytes.extend_from_slice(&COMPRESSED_MARKER);
        for unit in value.encode_utf16() {
            check(budget, cancelled)?;
            bytes.push(unit as u8);
        }
    } else {
        let escaped = value.starts_with('\u{feff}');
        if escaped {
            bytes.extend_from_slice(&ESCAPE_PREFIX);
        }
        for unit in value.encode_utf16() {
            check(budget, cancelled)?;
            if escaped && unit.to_le_bytes()[0] == WIDE_SWITCH {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
    }
    // A leading U+FEFF aliases the compression marker. Reopen must retain the
    // exact borrowed input under the same native decoder as existing callers.
    if text::decode(&bytes, true, budget)? != value {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
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
    let raw_length = raw_text_length(value_text, units)?;
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
    match value {
        Value::Null => Ok(Vec::new()),
        Value::Binary(bytes) => encode_for_span(field, value, bytes.len(), budget, cancelled),
        Value::Text(text) => encode_text_preferred(field, text, budget, cancelled),
        _ => Err(EditError::Unsupported(WriteBlocker::Column)),
    }
}

/// Borrowed Short Text representation below each caller's declaration admission.
pub(in crate::table::mutation) fn encode_text_preferred(
    field: &NativeColumn,
    value: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    check(budget, cancelled)?;
    let count = value.encode_utf16().count();
    if field.native_type() != SHORT_TEXT || count > field.storage_length() / UTF16_BYTES {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }
    let compact = field.extended_flags() & UNICODE_COMPRESSION_FLAG != 0
        && count > COMPRESSED_MARKER.len()
        && value
            .encode_utf16()
            .all(|unit| unit > 0 && unit <= u16::from(u8::MAX));
    let length = if compact {
        COMPRESSED_MARKER.len() + count
    } else {
        raw_text_length(value, count)?
    };
    // Preserve the complete-row raw UTF16 choice for the two-unit tie.
    if field.extended_flags() & UNICODE_COMPRESSION_FLAG != 0
        && count == COMPRESSED_MARKER.len()
        && !value.starts_with('\u{feff}')
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
        if text::decode(&bytes, true, budget)? != value {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        return Ok(bytes);
    }
    encode_text_for_span(field, value, length, budget, cancelled)
}

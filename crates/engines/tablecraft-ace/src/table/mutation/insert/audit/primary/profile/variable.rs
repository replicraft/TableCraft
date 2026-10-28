//! Independent component framing around the qualified primitive Text weights.
use super::super::reserve;
use crate::{EditError, NativeColumn, encode_text_index_key, format::reader::Reader};
use std::io::Cursor;
use tablecraft_core::values::Value;

const SEGMENT_BYTES: usize = 8;
const TAG_BYTES: usize = size_of::<u8>();
const CONTINUED: u8 = 9;
const ASC_NULL: u8 = 0;
const DESC_NULL: u8 = 0xff;
const ASC_VALUE: u8 = 0x7f;
const DESC_VALUE: u8 = 0x80;
const ASC_PADDING: u8 = 0;
const DESC_PADDING: u8 = 0xff;
pub(super) const MAX_SHORT_BINARY: usize = 510;
pub(super) const MAX_SHORT_TEXT_UNITS: usize = 255;
pub(super) const UTF16_BYTES: usize = size_of::<u16>();

pub(super) fn append(
    bytes: &mut Vec<u8>,
    data: &[u8],
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    reserve(bytes, data.len(), reader)?;
    reader.work(data.len())?;
    bytes.extend_from_slice(data);
    Ok(())
}

pub(super) fn text(
    field: &NativeColumn,
    descending: bool,
    value: &Value,
    bytes: &mut Vec<u8>,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    let text = match value {
        Value::Null => {
            return append(
                bytes,
                &[if descending { DESC_NULL } else { ASC_NULL }],
                reader,
            );
        }
        Value::Text(text) => text,
        _ => return Err(EditError::Preservation),
    };
    let declared = field.storage_length();
    if !declared.is_multiple_of(UTF16_BYTES) || declared / UTF16_BYTES > MAX_SHORT_TEXT_UNITS {
        return Err(EditError::Preservation);
    }
    for (position, _) in text.encode_utf16().enumerate() {
        reader.tick()?;
        if position >= declared / UTF16_BYTES {
            return Err(EditError::Preservation);
        }
    }
    let order = field.text_sort_order().ok_or(EditError::Preservation)?;
    let weights = encode_text_index_key(order, text, descending, reader.budget, reader.cancelled)?;
    append(
        bytes,
        &[if descending { DESC_VALUE } else { ASC_VALUE }],
        reader,
    )?;
    append(bytes, weights.as_bytes(), reader)
}

pub(super) fn binary(
    maximum: usize,
    descending: bool,
    value: &Value,
    bytes: &mut Vec<u8>,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    if !(1..=MAX_SHORT_BINARY).contains(&maximum) {
        return Err(EditError::Preservation);
    }
    let payload = match value {
        Value::Null => {
            return append(
                bytes,
                &[if descending { DESC_NULL } else { ASC_NULL }],
                reader,
            );
        }
        Value::Binary(value) if value.len() <= maximum => value,
        _ => return Err(EditError::Preservation),
    };
    append(
        bytes,
        &[if descending { DESC_VALUE } else { ASC_VALUE }],
        reader,
    )?;
    let segments = payload.len().div_ceil(SEGMENT_BYTES);
    for (position, part) in payload.chunks(SEGMENT_BYTES).enumerate() {
        let mut segment = [if descending {
            DESC_PADDING
        } else {
            ASC_PADDING
        }; SEGMENT_BYTES + TAG_BYTES];
        for (target, source) in segment.iter_mut().zip(part) {
            *target = if descending { !*source } else { *source };
        }
        segment[SEGMENT_BYTES] = if position + 1 < segments {
            CONTINUED
        } else if descending {
            !(part.len() as u8)
        } else {
            part.len() as u8
        };
        append(bytes, &segment, reader)?;
    }
    Ok(())
}

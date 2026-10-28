//! Independent native GUID segments; the continuation tag is never inverted.
use super::super::Wire;
use crate::{EditError, format::reader::Reader};
use std::io::Cursor;
use tablecraft_core::values::{Guid, Value};

const MARKER_BYTES: usize = size_of::<u8>();
const TAG_BYTES: usize = size_of::<u8>();
const SEGMENT_COUNT: usize = 2;
const SEGMENT_BYTES: usize = Guid::BYTE_LEN / SEGMENT_COUNT;
const KEY_BYTES: usize = MARKER_BYTES + Guid::BYTE_LEN + SEGMENT_COUNT * TAG_BYTES;
const ROW_BYTES: usize = size_of::<u32>();
const ASC_VALUE: u8 = 0x7f;
const DESC_VALUE: u8 = 0x80;
const ASC_NULL: u8 = 0;
const DESC_NULL: u8 = 0xff;
const CONTINUED: u8 = 9;
const COMPLETE: u8 = SEGMENT_BYTES as u8;

pub(super) fn encode(
    value: &Value,
    descending: bool,
    row: u32,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Wire, EditError> {
    let mut bytes = [0; KEY_BYTES + ROW_BYTES];
    let length = match value {
        Value::Null => {
            bytes[0] = if descending { DESC_NULL } else { ASC_NULL };
            MARKER_BYTES
        }
        Value::Guid(value) => {
            let canonical = value.to_bytes();
            bytes[0] = if descending { DESC_VALUE } else { ASC_VALUE };
            for (position, byte) in canonical.iter().enumerate() {
                let target =
                    MARKER_BYTES + position + usize::from(position >= SEGMENT_BYTES) * TAG_BYTES;
                bytes[target] = if descending { !byte } else { *byte };
            }
            bytes[MARKER_BYTES + SEGMENT_BYTES] = CONTINUED;
            bytes[KEY_BYTES - TAG_BYTES] = if descending { !COMPLETE } else { COMPLETE };
            KEY_BYTES
        }
        _ => return Err(EditError::Preservation),
    };
    bytes[length..length + ROW_BYTES].copy_from_slice(&row.to_be_bytes());
    Wire::read_paid(&bytes[..length + ROW_BYTES], reader)
}

pub(super) fn validate(bytes: &[u8], descending: bool) -> Result<(bool, usize), EditError> {
    let marker = *bytes.first().ok_or(EditError::Preservation)?;
    if marker == if descending { DESC_NULL } else { ASC_NULL } {
        return Ok((true, MARKER_BYTES));
    }
    let key = bytes.get(..KEY_BYTES).ok_or(EditError::Preservation)?;
    if marker != if descending { DESC_VALUE } else { ASC_VALUE }
        || key[MARKER_BYTES + SEGMENT_BYTES] != CONTINUED
        || key[KEY_BYTES - TAG_BYTES] != if descending { !COMPLETE } else { COMPLETE }
    {
        return Err(EditError::Preservation);
    }
    Ok((false, KEY_BYTES))
}

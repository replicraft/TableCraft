//! Fixed GUID index segments; canonical field bytes and SQL ordering stay separate.
use super::{
    key::{ASC_NULL, ASC_VALUE, DESC_NULL, DESC_VALUE, KeyProfile, MARKER_BYTES},
    page::{CHILD_BYTES, ROW_BYTES},
};
use crate::InspectError;
use crate::table::indexes::key::KeyRow;
use std::cmp::Ordering;
use tablecraft_core::values::{Guid, Value};

const SEGMENT_BYTES: usize = 8;
const TAG_BYTES: usize = size_of::<u8>();
const CONTINUED_SEGMENT: u8 = 9;
const FIRST_TAG: usize = MARKER_BYTES + SEGMENT_BYTES;
const LAST_DATA: usize = FIRST_TAG + TAG_BYTES;
const LAST_TAG: usize = LAST_DATA + SEGMENT_BYTES;
pub(super) const KEY_BYTES: usize = LAST_TAG + TAG_BYTES;
pub(super) const MAX_ENTRY_BYTES: usize = KEY_BYTES + ROW_BYTES + CHILD_BYTES;

pub(super) struct GuidProfile {
    pub column: usize,
    pub descending: bool,
}
impl KeyProfile for GuidProfile {
    type Key = Option<[u8; Guid::BYTE_LEN]>;
    fn components(&self) -> usize {
        1
    }
    fn max_key_bytes(&self) -> usize {
        KEY_BYTES
    }
    fn decode(&self, bytes: &[u8]) -> Result<(Self::Key, usize), InspectError> {
        decode_component(bytes, self.descending)
    }
    fn compare(&self, left: Self::Key, right: Self::Key) -> Ordering {
        let order = left.cmp(&right);
        if self.descending {
            order.reverse()
        } else {
            order
        }
    }
    fn row_key(&self, row: &(impl KeyRow + ?Sized)) -> Result<Self::Key, InspectError> {
        value_key(row.get(self.column).ok_or(InspectError::Malformed)?)
    }
    fn all_null(&self, key: Self::Key) -> bool {
        key.is_none()
    }
    fn any_null(&self, key: Self::Key) -> bool {
        key.is_none()
    }
}

pub(super) fn decode_component(
    bytes: &[u8],
    descending: bool,
) -> Result<(Option<[u8; Guid::BYTE_LEN]>, usize), InspectError> {
    let marker = *bytes.first().ok_or(InspectError::Malformed)?;
    if marker == if descending { DESC_NULL } else { ASC_NULL } {
        return Ok((None, MARKER_BYTES));
    }
    if marker != if descending { DESC_VALUE } else { ASC_VALUE } {
        return Err(InspectError::Malformed);
    }
    let encoded = bytes.get(..KEY_BYTES).ok_or(InspectError::Malformed)?;
    let final_tag = SEGMENT_BYTES as u8;
    if encoded[FIRST_TAG] != CONTINUED_SEGMENT
        || encoded[LAST_TAG] != if descending { !final_tag } else { final_tag }
    {
        return Err(InspectError::Malformed);
    }
    let mut canonical = [0; Guid::BYTE_LEN];
    canonical[..SEGMENT_BYTES].copy_from_slice(&encoded[MARKER_BYTES..FIRST_TAG]);
    canonical[SEGMENT_BYTES..].copy_from_slice(&encoded[LAST_DATA..LAST_TAG]);
    if descending {
        for byte in &mut canonical {
            *byte = !*byte;
        }
    }
    Ok((Some(canonical), KEY_BYTES))
}
pub(super) fn value_key(value: &Value) -> Result<Option<[u8; Guid::BYTE_LEN]>, InspectError> {
    match value {
        Value::Null => Ok(None),
        Value::Guid(value) => Ok(Some(value.to_bytes())),
        _ => Err(InspectError::Malformed),
    }
}

// Fixed two-segment representation from the pinned IndexData general-binary
// codec. The continuation tag is not complemented; the final length tag is.
pub(super) fn encode_component(
    key: Option<[u8; Guid::BYTE_LEN]>,
    descending: bool,
    bytes: &mut [u8],
) -> Result<usize, InspectError> {
    *bytes.first_mut().ok_or(InspectError::Malformed)? = match (key, descending) {
        (None, false) => ASC_NULL,
        (None, true) => DESC_NULL,
        (Some(_), false) => ASC_VALUE,
        (Some(_), true) => DESC_VALUE,
    };
    let Some(mut canonical) = key else {
        return Ok(MARKER_BYTES);
    };
    let encoded = bytes.get_mut(..KEY_BYTES).ok_or(InspectError::Malformed)?;
    if descending {
        for byte in &mut canonical {
            *byte = !*byte;
        }
    }
    encoded[MARKER_BYTES..FIRST_TAG].copy_from_slice(&canonical[..SEGMENT_BYTES]);
    encoded[FIRST_TAG] = CONTINUED_SEGMENT;
    encoded[LAST_DATA..LAST_TAG].copy_from_slice(&canonical[SEGMENT_BYTES..]);
    encoded[LAST_TAG] = if descending {
        !(SEGMENT_BYTES as u8)
    } else {
        SEGMENT_BYTES as u8
    };
    Ok(KEY_BYTES)
}

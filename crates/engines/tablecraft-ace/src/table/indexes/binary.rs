//! Bounded native Short Binary keys; no text collation or SQL coercion.
use super::{
    key::{ASC_NULL, ASC_VALUE, DESC_NULL, DESC_VALUE, KeyProfile, KeyRow, MARKER_BYTES},
    page::{CHILD_BYTES, ROW_BYTES},
};
use crate::{InspectError, format::data_type::MAX_SHORT_BINARY_BYTES};
use std::cmp::Ordering;
use tablecraft_core::values::Value;

const SEGMENT_BYTES: usize = 8;
const ENCODED_SEGMENT_BYTES: usize = SEGMENT_BYTES + size_of::<u8>();
const CONTINUED_SEGMENT: u8 = 9;
pub(super) const MAX_ENTRY_BYTES: usize = MARKER_BYTES
    + MAX_SHORT_BINARY_BYTES.div_ceil(SEGMENT_BYTES) * ENCODED_SEGMENT_BYTES
    + ROW_BYTES
    + CHILD_BYTES;

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) struct BinaryKey {
    bytes: [u8; MAX_SHORT_BINARY_BYTES],
    length: u16,
}
impl BinaryKey {
    fn empty() -> Self {
        Self {
            bytes: [0; MAX_SHORT_BINARY_BYTES],
            length: 0,
        }
    }
    fn bytes(&self) -> &[u8] {
        &self.bytes[..usize::from(self.length)]
    }
}
impl Ord for BinaryKey {
    fn cmp(&self, other: &Self) -> Ordering {
        self.bytes().cmp(other.bytes())
    }
}
impl PartialOrd for BinaryKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

pub(super) struct BinaryProfile {
    pub column: usize,
    pub descending: bool,
    pub maximum: usize,
}
impl KeyProfile for BinaryProfile {
    type Key = Option<BinaryKey>;
    fn components(&self) -> usize {
        1
    }
    fn max_key_bytes(&self) -> usize {
        MARKER_BYTES + self.maximum.div_ceil(SEGMENT_BYTES) * ENCODED_SEGMENT_BYTES
    }
    fn decode(&self, bytes: &[u8]) -> Result<(Self::Key, usize), InspectError> {
        let marker = *bytes.first().ok_or(InspectError::Malformed)?;
        if marker == if self.descending { DESC_NULL } else { ASC_NULL } {
            return Ok((None, MARKER_BYTES));
        }
        if marker
            != if self.descending {
                DESC_VALUE
            } else {
                ASC_VALUE
            }
        {
            return Err(InspectError::Malformed);
        }
        let mut key = BinaryKey::empty();
        let mut position = MARKER_BYTES;
        // Empty Binary is a value marker alone; the caller strips row/child trailers.
        if bytes.len() == position {
            return Ok((Some(key), position));
        }
        loop {
            let segment = bytes
                .get(position..position + ENCODED_SEGMENT_BYTES)
                .ok_or(InspectError::Malformed)?;
            let tag = segment[SEGMENT_BYTES];
            let continued = tag == CONTINUED_SEGMENT;
            let count = if continued {
                SEGMENT_BYTES
            } else {
                usize::from(if self.descending { !tag } else { tag })
            };
            if count == 0 || count > SEGMENT_BYTES || usize::from(key.length) + count > self.maximum
            {
                return Err(InspectError::Malformed);
            }
            let start = usize::from(key.length);
            for (to, from) in key.bytes[start..start + count].iter_mut().zip(segment) {
                *to = if self.descending { !from } else { *from };
            }
            if segment[count..SEGMENT_BYTES]
                .iter()
                .any(|byte| *byte != if self.descending { u8::MAX } else { 0 })
            {
                return Err(InspectError::Malformed);
            }
            key.length += count as u16;
            position += ENCODED_SEGMENT_BYTES;
            if !continued {
                return Ok((Some(key), position));
            }
        }
    }
    fn compare(&self, left: Self::Key, right: Self::Key) -> Ordering {
        if let (Some(left), Some(right)) = (left, right)
            && (left.length == 0 || right.length == 0)
        {
            // Empty is a strict encoded prefix of nonempty values in either direction.
            return left.length.cmp(&right.length);
        }
        let order = left.cmp(&right);
        if self.descending {
            order.reverse()
        } else {
            order
        }
    }
    fn row_key(&self, row: &(impl KeyRow + ?Sized)) -> Result<Self::Key, InspectError> {
        match row.get(self.column).ok_or(InspectError::Malformed)? {
            Value::Null => Ok(None),
            Value::Binary(value) if value.len() <= self.maximum => {
                let mut key = BinaryKey::empty();
                key.bytes[..value.len()].copy_from_slice(value);
                key.length = value.len() as u16;
                Ok(Some(key))
            }
            _ => Err(InspectError::Malformed),
        }
    }
    fn all_null(&self, key: Self::Key) -> bool {
        key.is_none()
    }
    fn any_null(&self, key: Self::Key) -> bool {
        key.is_none()
    }
}

#[cfg(test)]
#[path = "binary/tests.rs"]
mod tests;

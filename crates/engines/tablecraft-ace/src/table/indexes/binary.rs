//! Bounded native Short Binary keys; no text collation or SQL coercion.
use super::{
    key::{ASC_NULL, ASC_VALUE, DESC_NULL, DESC_VALUE, KeyProfile, KeyRow, MARKER_BYTES},
    page::{CHILD_BYTES, ROW_BYTES},
};
use crate::table::indexes::key::KeyWork;
use crate::{InspectError, format::data_type::MAX_SHORT_BINARY_BYTES};
use std::cmp::Ordering;
use tablecraft_core::values::Value;

mod selection;
pub use selection::BinaryIndexSelection;

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

pub(super) struct BinaryProfile<'a> {
    pub column: usize,
    pub descending: bool,
    pub maximum: usize,
    pub selection: BinaryIndexSelection<'a>,
}
impl KeyProfile for BinaryProfile<'_> {
    type Key = Option<BinaryKey>;
    fn components(&self) -> usize {
        1
    }
    fn max_key_bytes(&self) -> usize {
        MARKER_BYTES + self.maximum.div_ceil(SEGMENT_BYTES) * ENCODED_SEGMENT_BYTES
    }
    fn decode(
        &self,
        bytes: &[u8],
        _work: &mut KeyWork<'_>,
    ) -> Result<(Self::Key, usize), InspectError> {
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
    fn compare(&self, left: &Self::Key, right: &Self::Key) -> Ordering {
        compare_bytes(
            left.as_ref().map(BinaryKey::bytes),
            right.as_ref().map(BinaryKey::bytes),
            self.descending,
        )
    }
    fn row_key(
        &self,
        row: &(impl KeyRow + ?Sized),
        _work: &mut KeyWork<'_>,
    ) -> Result<Self::Key, InspectError> {
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
    fn constraint_nulls(&self, key: &Self::Key) -> Result<(bool, bool), InspectError> {
        Ok((key.is_none(), key.is_none()))
    }
    fn contains(&self, key: &Self::Key) -> bool {
        self.selection.contains(*key)
    }
    fn complete(&self) -> bool {
        self.selection == BinaryIndexSelection::All
    }
    fn may_match(
        &self,
        lower: Option<&super::page::KeyRef<Self::Key>>,
        upper: Option<&super::page::KeyRef<Self::Key>>,
    ) -> bool {
        self.selection.may_match(
            lower.copied(),
            upper.copied(),
            self.descending,
            self.maximum,
        )
    }
}

/// Complete binary component, before a containing text tuple's entry digest.
pub(super) fn encode_component(
    value: &Value,
    maximum: usize,
    descending: bool,
    output: &mut [u8],
) -> Result<usize, InspectError> {
    let bytes = match value {
        Value::Null => {
            *output.first_mut().ok_or(InspectError::Malformed)? =
                if descending { DESC_NULL } else { ASC_NULL };
            return Ok(MARKER_BYTES);
        }
        Value::Binary(bytes) if bytes.len() <= maximum => bytes.as_slice(),
        _ => return Err(InspectError::Malformed),
    };
    *output.first_mut().ok_or(InspectError::Malformed)? =
        if descending { DESC_VALUE } else { ASC_VALUE };
    let mut end = MARKER_BYTES;
    for (position, chunk) in bytes.chunks(SEGMENT_BYTES).enumerate() {
        let segment = output
            .get_mut(end..end + ENCODED_SEGMENT_BYTES)
            .ok_or(InspectError::Malformed)?;
        segment.fill(if descending { u8::MAX } else { 0 });
        for (to, from) in segment.iter_mut().zip(chunk) {
            *to = if descending { !from } else { *from };
        }
        segment[SEGMENT_BYTES] = if (position + 1) * SEGMENT_BYTES < bytes.len() {
            CONTINUED_SEGMENT
        } else if descending {
            !(chunk.len() as u8)
        } else {
            chunk.len() as u8
        };
        end += ENCODED_SEGMENT_BYTES;
    }
    Ok(end)
}

fn compare_bytes(left: Option<&[u8]>, right: Option<&[u8]>, descending: bool) -> Ordering {
    if let (Some(left), Some(right)) = (left, right)
        && (left.is_empty() || right.is_empty())
    {
        return left.len().cmp(&right.len());
    }
    let order = left.cmp(&right);
    if descending { order.reverse() } else { order }
}

#[cfg(test)]
#[path = "binary/tests.rs"]
mod tests;

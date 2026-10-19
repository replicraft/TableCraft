//! Private physical key policies; these do not define SQL comparison/coercion.
use super::{Int64IndexSelection, page::KeyRef, profile::KeyType};
use crate::InspectError;
use std::cmp::Ordering;
use tablecraft_core::values::Value;

pub(super) const MARKER_BYTES: usize = size_of::<u8>();
pub(super) const ASC_VALUE: u8 = 0x7f;
pub(super) const DESC_VALUE: u8 = 0x80;
pub(super) const ASC_NULL: u8 = 0;
pub(super) const DESC_NULL: u8 = 0xff;
pub(super) const INT32_SIGN_BIT: u32 = 1 << (u32::BITS - 1);
const INT64_SIGN_BIT: u64 = 1 << (u64::BITS - 1);
const INT16_SIGN_BIT: u16 = 1 << (u16::BITS - 1);
const MAX_SCALAR_BYTES: usize = size_of::<i64>();

pub(super) trait KeyProfile {
    type Key: Copy + Eq;
    fn components(&self) -> usize;
    fn max_key_bytes(&self) -> usize;
    fn decode(&self, bytes: &[u8]) -> Result<(Self::Key, usize), InspectError>;
    fn compare(&self, left: Self::Key, right: Self::Key) -> Ordering;
    fn row_key(&self, row: &[Value]) -> Result<Self::Key, InspectError>;
    fn all_null(&self, key: Self::Key) -> bool;
    fn any_null(&self, key: Self::Key) -> bool;
    fn complete(&self) -> bool {
        true
    }
    fn contains(&self, _key: Self::Key) -> bool {
        true
    }
    fn may_match(
        &self,
        _lower: Option<KeyRef<Self::Key>>,
        _upper: Option<KeyRef<Self::Key>>,
    ) -> bool {
        true
    }
}

pub(super) struct SingleKey {
    pub column: usize,
    pub key_type: KeyType,
    pub descending: bool,
    pub selection: Int64IndexSelection,
}

impl KeyProfile for SingleKey {
    type Key = Option<i64>;
    fn components(&self) -> usize {
        1
    }
    fn max_key_bytes(&self) -> usize {
        MARKER_BYTES + self.key_type.bytes()
    }
    fn decode(&self, bytes: &[u8]) -> Result<(Self::Key, usize), InspectError> {
        decode_component(bytes, self.key_type, self.descending)
    }
    fn compare(&self, left: Self::Key, right: Self::Key) -> Ordering {
        let order = left.cmp(&right);
        if self.descending {
            order.reverse()
        } else {
            order
        }
    }
    fn row_key(&self, row: &[Value]) -> Result<Self::Key, InspectError> {
        self.key_type
            .key(row.get(self.column).ok_or(InspectError::Malformed)?)
    }
    fn all_null(&self, key: Self::Key) -> bool {
        key.is_none()
    }
    fn any_null(&self, key: Self::Key) -> bool {
        key.is_none()
    }
    fn contains(&self, key: Self::Key) -> bool {
        self.selection.contains(key)
    }
    fn complete(&self) -> bool {
        self.selection == Int64IndexSelection::All
    }
    fn may_match(&self, lower: Option<KeyRef>, upper: Option<KeyRef>) -> bool {
        self.selection.may_match(lower, upper, self.descending)
    }
}

pub(super) fn decode_component(
    bytes: &[u8],
    key_type: KeyType,
    descending: bool,
) -> Result<(Option<i64>, usize), InspectError> {
    let marker = *bytes.first().ok_or(InspectError::Malformed)?;
    if marker == if descending { DESC_NULL } else { ASC_NULL } {
        return Ok((None, MARKER_BYTES));
    }
    if marker != if descending { DESC_VALUE } else { ASC_VALUE } {
        return Err(InspectError::Malformed);
    }
    let end = MARKER_BYTES + key_type.bytes();
    let raw = bytes
        .get(MARKER_BYTES..end)
        .ok_or(InspectError::Malformed)?;
    let mut decoded = [0; MAX_SCALAR_BYTES];
    let decoded = &mut decoded[..key_type.bytes()];
    decoded.copy_from_slice(raw);
    if descending {
        for byte in &mut *decoded {
            *byte = !*byte;
        }
    }
    let key = match key_type {
        KeyType::Byte => i64::from(decoded[0]),
        KeyType::Int16 => i64::from(
            (u16::from_be_bytes(decoded.try_into().map_err(|_| InspectError::Malformed)?)
                ^ INT16_SIGN_BIT) as i16,
        ),
        KeyType::Int32 => i64::from(
            (u32::from_be_bytes(decoded.try_into().map_err(|_| InspectError::Malformed)?)
                ^ INT32_SIGN_BIT) as i32,
        ),
        KeyType::Int64 | KeyType::Currency => {
            (u64::from_be_bytes(decoded.try_into().map_err(|_| InspectError::Malformed)?)
                ^ INT64_SIGN_BIT) as i64
        }
    };
    Ok((Some(key), end))
}

//! Shared read/write key policies; single keys keep their compact representation.
use super::super::key::{KeyProfile, MARKER_BYTES, decode_component};
use super::super::{composite::MixedValue, guid::GuidProfile};
use super::NumericKey;
use crate::{EditError, InspectError};
use std::cmp::Ordering;
use tablecraft_core::values::Value;

pub(super) trait WriteProfile: KeyProfile {
    fn requires_repack(&self) -> bool {
        false
    }
    fn includes(&self, column: usize) -> bool;
    fn encode(&self, key: Self::Key, bytes: &mut [u8]) -> Result<usize, InspectError>;
    fn update(
        &self,
        key: &mut Self::Key,
        column: usize,
        expected: Option<MixedValue>,
        value: Option<MixedValue>,
    ) -> Result<(), EditError>;
}

pub(super) struct SingleProfile {
    pub column: usize,
    pub kind: NumericKey,
    pub descending: bool,
}
impl KeyProfile for SingleProfile {
    type Key = Option<i64>;
    fn components(&self) -> usize {
        1
    }
    fn max_key_bytes(&self) -> usize {
        MARKER_BYTES + self.kind.bytes()
    }
    fn decode(&self, bytes: &[u8]) -> Result<(Self::Key, usize), InspectError> {
        decode_component(bytes, self.kind.key_type(), self.descending)
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
        self.kind
            .key_type()
            .key(row.get(self.column).ok_or(InspectError::Malformed)?)
    }
    fn all_null(&self, key: Self::Key) -> bool {
        key.is_none()
    }
    fn any_null(&self, key: Self::Key) -> bool {
        key.is_none()
    }
}
impl WriteProfile for SingleProfile {
    fn requires_repack(&self) -> bool {
        self.kind.requires_repack()
    }
    fn includes(&self, column: usize) -> bool {
        column == self.column
    }
    fn encode(&self, key: Self::Key, bytes: &mut [u8]) -> Result<usize, InspectError> {
        super::encoding::component(key, self.kind, self.descending, bytes)
    }
    fn update(
        &self,
        key: &mut Self::Key,
        column: usize,
        expected: Option<MixedValue>,
        value: Option<MixedValue>,
    ) -> Result<(), EditError> {
        if !self.includes(column) {
            return Err(InspectError::Malformed.into());
        }
        let expected = super::kind::numeric(expected)?;
        let value = super::kind::numeric(value)?;
        if *key != expected {
            return Err(EditError::Conflict);
        }
        *key = value;
        Ok(())
    }
}

impl WriteProfile for GuidProfile {
    fn requires_repack(&self) -> bool {
        true
    }
    fn includes(&self, column: usize) -> bool {
        self.column == column
    }
    fn encode(&self, key: Self::Key, bytes: &mut [u8]) -> Result<usize, InspectError> {
        super::super::guid::encode_component(key, self.descending, bytes)
    }
    fn update(
        &self,
        key: &mut Self::Key,
        column: usize,
        expected: Option<MixedValue>,
        value: Option<MixedValue>,
    ) -> Result<(), EditError> {
        if !self.includes(column) {
            return Err(InspectError::Malformed.into());
        }
        if *key != super::kind::guid_key(expected)? {
            return Err(EditError::Conflict);
        }
        *key = super::kind::guid_key(value)?;
        Ok(())
    }
}

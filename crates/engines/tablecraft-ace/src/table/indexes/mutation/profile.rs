//! Shared read/write key policies; single keys keep their compact representation.
use super::super::key::{KeyProfile, MARKER_BYTES, decode_component};
use super::IntegralKey;
use crate::{EditError, InspectError};
use std::cmp::Ordering;
use tablecraft_core::values::Value;

pub(super) trait WriteProfile: KeyProfile {
    fn includes(&self, column: usize) -> bool;
    fn encode(&self, key: Self::Key, bytes: &mut [u8]) -> Result<usize, InspectError>;
    fn update(
        &self,
        key: &mut Self::Key,
        column: usize,
        expected: Option<i64>,
        value: Option<i64>,
    ) -> Result<(), EditError>;
}

pub(super) struct SingleProfile {
    pub column: usize,
    pub kind: IntegralKey,
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
        expected: Option<i64>,
        value: Option<i64>,
    ) -> Result<(), EditError> {
        if !self.includes(column) {
            return Err(InspectError::Malformed.into());
        }
        if *key != expected {
            return Err(EditError::Conflict);
        }
        *key = value;
        Ok(())
    }
}

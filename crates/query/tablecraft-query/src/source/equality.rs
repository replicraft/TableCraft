use super::TextCollationId;
use crate::QueryError;

/// Host-issued conservative equality key in the source's text-policy namespace.
/// The query engine never interprets these bytes. The issuing resolver must
/// guarantee that its comparer-equal strings cannot be excluded by the matching
/// source operation. Keys are execution-local hints, not portable stored data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TextEqualityKey {
    policy: TextCollationId,
    length: u8,
    bytes: [u8; Self::MAX_BYTES],
}

impl TextEqualityKey {
    /// Bounded opaque payload envelope for current source equality hints.
    pub const MAX_BYTES: usize = 32;

    /// Retain a bounded host payload without inferring or qualifying its meaning.
    pub fn from_host(policy: TextCollationId, bytes: &[u8]) -> Result<Self, QueryError> {
        if bytes.len() > Self::MAX_BYTES {
            return Err(QueryError::Source);
        }

        let mut key = Self {
            policy,
            length: bytes.len() as u8,
            bytes: [0; Self::MAX_BYTES],
        };
        key.bytes[..bytes.len()].copy_from_slice(bytes);
        Ok(key)
    }

    /// Policy whose actual equality semantics qualify this hint.
    pub const fn policy(self) -> TextCollationId {
        self.policy
    }

    /// Opaque bytes for the matching source implementation only.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..usize::from(self.length)]
    }
}

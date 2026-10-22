//! Exact GUID points in canonical bytes; no SQL GUID ordering is defined here.
use super::super::page::KeyRef;
use tablecraft_core::values::Guid;

/// Selection through a qualified single-GUID native index.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GuidIndexSelection {
    /// Complete traversal including NULL, retaining complete count checks.
    All,
    /// Only stored NULL keys, distinct from the all-zero GUID.
    Null,
    /// Every row with these exact canonical GUID bytes; excludes NULL.
    Equal(Guid),
}

impl GuidIndexSelection {
    pub(super) fn contains(self, key: Option<[u8; Guid::BYTE_LEN]>) -> bool {
        match self {
            Self::All => true,
            Self::Null => key.is_none(),
            Self::Equal(value) => key == Some(value.to_bytes()),
        }
    }

    pub(super) fn may_match(
        self,
        lower: Option<&KeyRef<Option<[u8; Guid::BYTE_LEN]>>>,
        upper: Option<&KeyRef<Option<[u8; Guid::BYTE_LEN]>>>,
        descending: bool,
    ) -> bool {
        let point = match self {
            Self::All => return true,
            Self::Null => None,
            Self::Equal(value) => Some(value.to_bytes()),
        };
        let (minimum, maximum) = if descending {
            (upper, lower)
        } else {
            (lower, upper)
        };
        // Fences also have row references. Inclusive key-only bounds preserve
        // duplicate GUID/NULL keys on either side of a separator.
        minimum.is_none_or(|entry| entry.key <= point)
            && maximum.is_none_or(|entry| entry.key >= point)
    }
}

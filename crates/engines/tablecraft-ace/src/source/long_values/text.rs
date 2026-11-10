use crate::{InspectError, LongValueReadLimits};

/// Decoded Memo UTF-8 allowance, independent of encoded storage and query limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LongTextReadLimits {
    maximum_bytes: u32,
}

impl LongTextReadLimits {
    /// A compressed single-byte UTF-16 unit needs at most two UTF-8 bytes;
    /// uncompressed UTF-16 requires no greater expansion per encoded byte.
    pub const MAX_BYTES: u32 = 2 * LongValueReadLimits::MAX_BYTES;
    /// Ordinary reads retain the complete native encoded-to-UTF-8 envelope.
    pub const DEFAULT_BYTES: u32 = Self::MAX_BYTES;

    /// Choose a positive decoded UTF-8 ceiling within the native decoding envelope.
    pub fn new(maximum_bytes: u32) -> Result<Self, InspectError> {
        if maximum_bytes == 0 || maximum_bytes > Self::MAX_BYTES {
            return Err(InspectError::InvalidLongTextLimits);
        }

        Ok(Self { maximum_bytes })
    }

    /// Maximum decoded UTF-8 bytes retained for one Memo value.
    pub const fn maximum_bytes(self) -> u32 {
        self.maximum_bytes
    }
}

impl Default for LongTextReadLimits {
    fn default() -> Self {
        Self {
            maximum_bytes: Self::DEFAULT_BYTES,
        }
    }
}

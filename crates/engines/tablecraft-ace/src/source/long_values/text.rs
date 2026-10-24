use crate::{InspectError, limits::MAX_LONG_TEXT_BYTES};

/// Decoded Memo UTF-8 allowance, independent of encoded storage and query limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LongTextReadLimits {
    maximum_bytes: u32,
}

impl LongTextReadLimits {
    /// Existing one-MiB decoded Memo ceiling.
    pub const DEFAULT_BYTES: u32 = MAX_LONG_TEXT_BYTES as u32;
    /// Explicit engineering ceiling, not a native format maximum.
    pub const MAX_BYTES: u32 = 16 * 1024 * 1024;

    /// Choose a positive decoded UTF-8 ceiling within the consumer envelope.
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

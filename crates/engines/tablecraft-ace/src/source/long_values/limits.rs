use crate::{
    InspectError,
    limits::{MAX_LONG_VALUE_BYTES, MAX_LONG_VALUE_CHUNKS},
};

/// Encoded MEMO/OLE storage allowance, independent of decoded-value policies.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LongValueReadLimits {
    maximum_bytes: u32,
}

impl LongValueReadLimits {
    /// Existing two-MiB encoded-value ceiling.
    pub const DEFAULT_BYTES: u32 = MAX_LONG_VALUE_BYTES as u32;
    /// Explicit engineering read ceiling, not a native format maximum.
    pub const MAX_BYTES: u32 = 32 * 1024 * 1024;

    /// Select a positive encoded-byte ceiling within the consumer envelope.
    /// Chain references scale by one default chunk allowance per two MiB.
    pub fn new(maximum_bytes: u32) -> Result<Self, InspectError> {
        if maximum_bytes == 0 || maximum_bytes > Self::MAX_BYTES {
            return Err(InspectError::InvalidLongValueLimits);
        }

        Ok(Self { maximum_bytes })
    }

    /// Maximum encoded bytes retained for one value.
    pub const fn maximum_bytes(self) -> u32 {
        self.maximum_bytes
    }

    /// Maximum physical row references followed for one value.
    pub const fn maximum_chunks(self) -> usize {
        self.maximum_bytes.div_ceil(Self::DEFAULT_BYTES) as usize * MAX_LONG_VALUE_CHUNKS
    }
}

impl Default for LongValueReadLimits {
    fn default() -> Self {
        Self {
            maximum_bytes: Self::DEFAULT_BYTES,
        }
    }
}

use crate::{InspectError, LongValueReadLimits};

/// Consumer file-size allowance, separate from encoded storage and header limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AttachmentDecodeLimits {
    maximum_file_bytes: u32,
    maximum_encoded_bytes: u32,
}

impl AttachmentDecodeLimits {
    /// Default one-MiB file allowance retained by existing entry points.
    pub const DEFAULT_FILE_BYTES: u32 = 1024 * 1024;
    /// Maximum explicit consumer file allowance; not a native format maximum.
    pub const MAX_FILE_BYTES: u32 = 16 * 1024 * 1024;

    /// Select a positive file ceiling within the qualified consumer envelope.
    /// This does not raise native encoded-value, header or cumulative budgets.
    pub fn new(maximum_file_bytes: u32) -> Result<Self, InspectError> {
        if maximum_file_bytes == 0 || maximum_file_bytes > Self::MAX_FILE_BYTES {
            return Err(InspectError::InvalidAttachmentLimits);
        }

        Ok(Self {
            maximum_file_bytes,
            maximum_encoded_bytes: LongValueReadLimits::DEFAULT_BYTES,
        })
    }

    /// Select an independent encoded envelope allowance, up to 32 MiB.
    /// The native source must separately permit reading that encoded value.
    pub fn with_encoded_bytes(mut self, maximum_bytes: u32) -> Result<Self, InspectError> {
        if maximum_bytes == 0 || maximum_bytes > LongValueReadLimits::MAX_BYTES {
            return Err(InspectError::InvalidAttachmentLimits);
        }

        self.maximum_encoded_bytes = maximum_bytes;
        Ok(self)
    }

    /// Maximum stored envelope bytes accepted by this decoder.
    pub const fn maximum_encoded_bytes(self) -> u32 {
        self.maximum_encoded_bytes
    }

    /// Maximum returned file bytes, excluding the native type header.
    pub const fn maximum_file_bytes(self) -> u32 {
        self.maximum_file_bytes
    }
}

impl Default for AttachmentDecodeLimits {
    fn default() -> Self {
        Self {
            maximum_file_bytes: Self::DEFAULT_FILE_BYTES,
            maximum_encoded_bytes: LongValueReadLimits::DEFAULT_BYTES,
        }
    }
}

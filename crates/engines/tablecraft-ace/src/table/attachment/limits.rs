use crate::{InspectError, LongValueReadLimits};

/// Consumer file-size allowance, separate from encoded storage and header limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AttachmentDecodeLimits {
    maximum_file_bytes: u32,
    maximum_encoded_bytes: u32,
}

impl AttachmentDecodeLimits {
    /// Microsoft Access limits each individual attachment to 256 MiB.
    /// <https://support.microsoft.com/en-us/access/attach-files-and-graphics-to-the-records-in-your-database>
    pub const MAX_FILE_BYTES: u32 = 256 * 1024 * 1024;
    /// Ordinary decoding admits the documented native file-size envelope.
    pub const DEFAULT_FILE_BYTES: u32 = Self::MAX_FILE_BYTES;

    /// Select an explicit lower file limit within the documented Access envelope.
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

    /// Select an explicit lower encoded limit within the native 30-bit envelope.
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

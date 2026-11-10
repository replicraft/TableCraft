use crate::InspectError;

/// Encoded MEMO/OLE storage allowance, independent of decoded-value policies.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LongValueReadLimits {
    maximum_bytes: u32,
}

impl LongValueReadLimits {
    /// The descriptor reserves its upper two bits for the storage kind.
    pub const MAX_BYTES: u32 = 0x3fff_ffff;
    /// Ordinary reads accept every length representable by the native descriptor.
    pub const DEFAULT_BYTES: u32 = Self::MAX_BYTES;

    /// Select an explicit positive caller limit within the native wire envelope.
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
}

impl Default for LongValueReadLimits {
    fn default() -> Self {
        Self {
            maximum_bytes: Self::DEFAULT_BYTES,
        }
    }
}

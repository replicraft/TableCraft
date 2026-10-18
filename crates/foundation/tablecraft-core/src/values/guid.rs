//! Canonical GUID values independent of native field encodings.

/// A GUID's canonical 16-byte sequence in textual group order.
///
/// Native GUID struct byte ordering is a codec concern. This type supplies no
/// Access index ordering, UUID generation, or conversion from semantic IDs.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct Guid([u8; Guid::BYTE_LEN]);

impl Guid {
    /// Number of bytes in the canonical 128-bit representation.
    pub const BYTE_LEN: usize = size_of::<u128>();
    /// Wrap canonical bytes; all 128-bit patterns are retained.
    pub const fn from_bytes(bytes: [u8; Guid::BYTE_LEN]) -> Self {
        Self(bytes)
    }

    /// Return canonical bytes, not an MDB/ACCDB field encoding.
    pub const fn to_bytes(self) -> [u8; Guid::BYTE_LEN] {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guid_bytes_stay_canonical_and_are_not_reinterpreted_as_native_layout() {
        let bytes = [
            0x6b, 0x29, 0xfc, 0x40, 0xca, 0x47, 0x10, 0x67, 0xb3, 0x1d, 0, 0xdd, 1, 6, 0x62, 0xda,
        ];
        assert_eq!(Guid::from_bytes(bytes).to_bytes(), bytes);
        assert_eq!(Guid::from_bytes([0; 16]).to_bytes(), [0; 16]);
    }
}

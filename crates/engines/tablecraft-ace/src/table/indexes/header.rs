//! Jet 4 / initial ACE index-page prefix. Semantic checks stay in the page decoder.
use crate::InspectError;
use zerocopy::{
    FromBytes, Immutable, KnownLayout, Unaligned,
    little_endian::{U16, U32},
};

#[derive(FromBytes, Immutable, KnownLayout, Unaligned)]
#[repr(C)]
pub(super) struct IndexHeader {
    pub kind: u8,
    pub version: u8,
    pub free: U16,
    pub owner: U32,
    _reserved: [u8; 4],
    pub previous: U32,
    pub next: U32,
    pub tail: U32,
    pub prefix: U16,
    _level: u8,
}

#[cfg(feature = "ace16-write-preview")]
pub(super) const LEVEL_OFFSET: usize = std::mem::offset_of!(IndexHeader, _level);

impl IndexHeader {
    // Native mutation requires a zero write stamp and the exact subtree height
    // observed in independent fixtures. Reads retain their broader profile.
    pub fn qualified_for_update(&self, height: u8) -> bool {
        self._reserved == [0; 4] && self._level == height
    }
    pub fn read(bytes: &[u8]) -> Result<&Self, InspectError> {
        Self::ref_from_prefix(bytes)
            .map(|(header, _)| header)
            .map_err(|_| InspectError::Truncated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefix_layout_is_little_endian_and_accepts_unaligned_input() {
        let bytes = [
            99, 4, 1, 0x34, 0x12, 0x78, 0x56, 0x34, 0x12, 0, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0, 3, 0,
            0, 0, 8, 0, 0,
        ];
        let header = IndexHeader::read(&bytes[1..]).unwrap();
        assert_eq!(
            (size_of::<IndexHeader>(), align_of::<IndexHeader>()),
            (27, 1)
        );
        assert_eq!((header.kind, header.version), (4, 1));
        assert_eq!(header.free.get(), 0x1234);
        assert_eq!(header.owner.get(), 0x12345678);
        assert_eq!(
            (header.previous.get(), header.next.get(), header.tail.get()),
            (1, 2, 3)
        );
        assert_eq!(header.prefix.get(), 8);
        for end in 0..27 {
            assert!(IndexHeader::read(&bytes[1..=end]).is_err());
        }
    }
}

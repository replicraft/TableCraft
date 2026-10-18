//! Native long-value descriptor; reserved bytes are preserved by the borrowed view.
use crate::InspectError;
use zerocopy::{FromBytes, Immutable, KnownLayout, Unaligned, little_endian::U32};

#[derive(FromBytes, Immutable, KnownLayout, Unaligned)]
#[repr(C)]
pub(super) struct Definition {
    pub tagged_length: U32,
    pub reference: U32,
    _reserved: [u8; 4],
}

impl Definition {
    pub fn read(bytes: &[u8]) -> Result<&Self, InspectError> {
        Self::ref_from_prefix(bytes)
            .map(|(definition, _)| definition)
            .map_err(|_| InspectError::Truncated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_width_and_unaligned_little_endian_fields_match_golden_bytes() {
        let bytes = [99, 3, 0, 0, 0x40, 7, 4, 3, 2, 9, 8, 7, 6];
        let definition = Definition::read(&bytes[1..]).unwrap();
        assert_eq!((size_of::<Definition>(), align_of::<Definition>()), (12, 1));
        assert_eq!(definition.tagged_length.get(), 0x40000003);
        assert_eq!(definition.reference.get(), 0x02030407);
        assert!(Definition::read(&bytes[1..12]).is_err());
    }
}

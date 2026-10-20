use crate::InspectError;

const ORDER_BYTES: usize = size_of::<u32>();
const LCID_BYTES: usize = size_of::<u16>();
const GENERAL_LCID: u16 = 1033;
const LEGACY_VERSION: u8 = 0;
const GENERAL_VERSION: u8 = 1;

/// Recognized General order family; recognition does not implement its weights.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeneralSortFamily {
    /// General legacy, used by Jet 4 / ACE 2007 defaults.
    Legacy,
    /// General, introduced by Access 2010 and used by later defaults.
    General2010,
}

/// Exact native text sort descriptor. Zero/unknown LCIDs and versions are retained;
/// no OS locale or format-family default is silently substituted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeSortOrder {
    bytes: [u8; ORDER_BYTES],
}
impl NativeSortOrder {
    /// Retain a caller-supplied native descriptor without substituting defaults.
    /// Every byte pattern is representable; recognition and key support are separate.
    pub const fn from_native_bytes(bytes: [u8; ORDER_BYTES]) -> Self {
        Self { bytes }
    }
    pub(crate) fn read(bytes: &[u8]) -> Result<Self, InspectError> {
        Ok(Self {
            bytes: bytes
                .get(..ORDER_BYTES)
                .ok_or(InspectError::Truncated)?
                .try_into()
                .unwrap(),
        })
    }
    /// Stored locale/collating-order identifier, including zero and unknown IDs.
    pub fn lcid(self) -> u16 {
        u16::from_le_bytes(self.bytes[..LCID_BYTES].try_into().unwrap())
    }
    /// Stored sort-table version, without inferring the creating Access build.
    pub const fn version(self) -> u8 {
        self.bytes[ORDER_BYTES - 1]
    }
    /// Stored sort variant; currently only variant zero is recognized.
    pub const fn variant(self) -> u8 {
        self.bytes[LCID_BYTES]
    }
    /// Recognize known General descriptors. This does not activate SQL comparison,
    /// LIKE or index traversal. Unspecified/unknown descriptors return None.
    pub fn general_family(self) -> Option<GeneralSortFamily> {
        if self.lcid() != GENERAL_LCID || self.variant() != 0 {
            return None;
        }
        match self.version() {
            LEGACY_VERSION => Some(GeneralSortFamily::Legacy),
            GENERAL_VERSION => Some(GeneralSortFamily::General2010),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_descriptors_do_not_guess_defaults_or_ignore_unknown_variants() {
        for (raw, family) in [
            ([9, 4, 0, 0], Some(GeneralSortFamily::Legacy)),
            ([9, 4, 0, 1], Some(GeneralSortFamily::General2010)),
            ([0, 0, 0, 1], None),
            ([31, 4, 0, 1], None),
            ([9, 4, 1, 1], None),
            ([9, 4, 0, 2], None),
            ([9, 4, 0, 255], None),
        ] {
            let order = NativeSortOrder::read(&raw).unwrap();
            assert_eq!(order.general_family(), family);
            assert_eq!(order.variant(), raw[2]);
            assert_eq!(order.version(), raw[3]);
        }
        for length in 0..4 {
            assert!(NativeSortOrder::read(&[0; 4][..length]).is_err());
        }
    }
}

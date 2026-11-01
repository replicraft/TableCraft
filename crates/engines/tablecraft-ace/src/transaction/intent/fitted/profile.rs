//! Closed data-record choices; no tag is a native allocation permission.
use super::codec::{MAGIC, PROFILE};

pub(in crate::transaction::intent) const OWNED_GROWTH_MAGIC: &[u8; 8] = b"TCNI0006";
const OWNED_GROWTH_PROFILE: u16 = 6;
pub(in crate::transaction::intent) const EOF_ALLOCATION_MAGIC: &[u8; 8] = b"TCNI0007";
const EOF_ALLOCATION_PROFILE: u16 = 7;

#[derive(Clone, Copy)]
pub(in crate::transaction::intent) enum WireProfile {
    Fitted,
    OwnedOnly,
    AllocateEof,
}

impl WireProfile {
    pub(super) const fn magic(self) -> &'static [u8; 8] {
        match self {
            Self::Fitted => MAGIC,
            Self::OwnedOnly => OWNED_GROWTH_MAGIC,
            Self::AllocateEof => EOF_ALLOCATION_MAGIC,
        }
    }

    pub(super) const fn number(self) -> u16 {
        match self {
            Self::Fitted => PROFILE,
            Self::OwnedOnly => OWNED_GROWTH_PROFILE,
            Self::AllocateEof => EOF_ALLOCATION_PROFILE,
        }
    }
}

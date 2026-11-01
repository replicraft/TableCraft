//! Closed data-record choices; neither tag is a native allocation permission.
use super::codec::{MAGIC, PROFILE};

pub(in crate::transaction::intent) const OWNED_GROWTH_MAGIC: &[u8; 8] = b"TCNI0006";
const OWNED_GROWTH_PROFILE: u16 = 6;

#[derive(Clone, Copy)]
pub(in crate::transaction::intent) enum WireProfile {
    Fitted,
    OwnedOnly,
}

impl WireProfile {
    pub(super) const fn magic(self) -> &'static [u8; 8] {
        match self {
            Self::Fitted => MAGIC,
            Self::OwnedOnly => OWNED_GROWTH_MAGIC,
        }
    }

    pub(super) const fn number(self) -> u16 {
        match self {
            Self::Fitted => PROFILE,
            Self::OwnedOnly => OWNED_GROWTH_PROFILE,
        }
    }
}

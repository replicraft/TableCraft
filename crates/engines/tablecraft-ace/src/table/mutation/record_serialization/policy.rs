//! Explicit operation-owned wire policies; caches never select a proof policy.
#[derive(Clone, Copy)]
pub(in crate::table::mutation) enum RecordPolicy {
    FittedDeclared,
    #[cfg(feature = "ace16-write-preview")]
    GrowingPreferred,
}

impl RecordPolicy {
    pub(in crate::table::mutation) fn prefers_growth(self) -> bool {
        match self {
            Self::FittedDeclared => false,
            #[cfg(feature = "ace16-write-preview")]
            Self::GrowingPreferred => true,
        }
    }
}

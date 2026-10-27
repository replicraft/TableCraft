//! Complete catalog map footprints, separate from writable map-cell authority.
mod catalog;
mod claims;

use crate::format::allocation::MapRole;
pub(in crate::table) use catalog::capture_catalog;
pub(in crate::table) use claims::{Census, RecordIdentity};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::table) enum Owner {
    GlobalFree,
    OrdinaryOwned { definition: u32 },
    OrdinaryCandidate { definition: u32 },
    IndexOwned { definition: u32, physical: u32 },
    ColumnOwned { definition: u32, column: usize },
    ColumnCandidate { definition: u32, column: usize },
}

impl Owner {
    fn grammar(self) -> MapRole {
        match self {
            Self::GlobalFree => MapRole::GlobalFree,
            Self::OrdinaryCandidate { .. } => MapRole::TableFreeSpace,
            Self::ColumnCandidate { .. } => MapRole::LongValueCandidate,
            Self::OrdinaryOwned { .. } | Self::IndexOwned { .. } | Self::ColumnOwned { .. } => {
                MapRole::Owned
            }
        }
    }
}

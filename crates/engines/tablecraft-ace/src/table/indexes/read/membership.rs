//! Explicit requests use bits; legacy complete consumers retain materialized gates.
use crate::{
    InspectError, NativeReadSource,
    format::{
        allocation::{self, PageMembership},
        reader::Reader,
    },
};

#[derive(Clone, Copy)]
pub(super) enum OwnershipStrategy {
    Materialized,
    Bitmap,
}

impl OwnershipStrategy {
    pub(super) fn for_read(complete: bool, legacy_table_limit: bool) -> Self {
        if complete && legacy_table_limit {
            Self::Materialized
        } else {
            Self::Bitmap
        }
    }
}

pub(super) enum DataPages {
    Materialized(Vec<u32>),
    Bitmap(PageMembership),
}

impl DataPages {
    pub(super) fn new<R: NativeReadSource>(
        reader: &mut Reader<'_, R>,
        reference: u32,
        strategy: OwnershipStrategy,
    ) -> Result<Self, InspectError> {
        match strategy {
            OwnershipStrategy::Materialized => {
                allocation::pages(reader, reference).map(Self::Materialized)
            }
            OwnershipStrategy::Bitmap => PageMembership::new(reader, reference).map(Self::Bitmap),
        }
    }

    pub(super) fn contains<R: NativeReadSource>(
        &mut self,
        reader: &mut Reader<'_, R>,
        number: u32,
    ) -> Result<bool, InspectError> {
        match self {
            Self::Materialized(pages) => Ok(pages.binary_search(&number).is_ok()),
            Self::Bitmap(pages) => pages.contains(reader, number),
        }
    }
}

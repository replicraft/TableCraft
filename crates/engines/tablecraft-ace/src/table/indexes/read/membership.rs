//! Complete consumers retain their old materialized-map gate; selections use bits.
use crate::{
    InspectError, NativeReadSource,
    format::{
        allocation::{self, PageMembership},
        reader::Reader,
    },
};

pub(super) enum DataPages {
    Complete(Vec<u32>),
    Selected(PageMembership),
}

impl DataPages {
    pub(super) fn new<R: NativeReadSource>(
        reader: &mut Reader<'_, R>,
        reference: u32,
        complete: bool,
    ) -> Result<Self, InspectError> {
        if complete {
            allocation::pages(reader, reference).map(Self::Complete)
        } else {
            PageMembership::new(reader, reference).map(Self::Selected)
        }
    }

    pub(super) fn contains<R: NativeReadSource>(
        &mut self,
        reader: &mut Reader<'_, R>,
        number: u32,
    ) -> Result<bool, InspectError> {
        match self {
            Self::Complete(pages) => Ok(pages.binary_search(&number).is_ok()),
            Self::Selected(pages) => pages.contains(reader, number),
        }
    }
}

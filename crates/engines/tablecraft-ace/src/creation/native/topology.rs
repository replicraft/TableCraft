//! Logical declaration length determines physical continuation and body roles.
use crate::{InspectError, creation::CreateDatabaseError, format::page::PAGE_BYTES};
use std::ops::Range;

pub(super) const DEFINITION_HEADER: usize = 8;
pub(super) const CHAIN_PAYLOAD: usize = PAGE_BYTES - 24;
pub(super) const BITMAP_OFFSET: usize = 4;
pub(super) const BITMAP_PAGES: usize = (PAGE_BYTES - BITMAP_OFFSET) * u8::BITS as usize;

pub(super) struct Topology {
    pub definition: Range<usize>,
    pub data: usize,
    pub index: Option<usize>,
    pub properties: Range<usize>,
    pub bitmaps: Range<usize>,
    pub end: usize,
}

impl Topology {
    pub fn capture(
        root: usize,
        logical_bytes: usize,
        property_bytes: usize,
        keyed: bool,
    ) -> Result<Self, CreateDatabaseError> {
        let continuation = logical_bytes
            .saturating_sub(PAGE_BYTES)
            .div_ceil(PAGE_BYTES - DEFINITION_HEADER);
        let data = root
            .checked_add(1 + continuation)
            .ok_or(CreateDatabaseError::PageCapacity)?;
        let index = keyed.then_some(data + 1);
        let property_start = data
            .checked_add(1 + usize::from(keyed))
            .ok_or(CreateDatabaseError::PageCapacity)?;
        let property_end = property_start
            .checked_add(property_bytes.div_ceil(CHAIN_PAYLOAD))
            .ok_or(CreateDatabaseError::PageCapacity)?;
        // An inline map is sufficient only when its actual represented window
        // contains every property page. Larger maps use native bitmap pages.
        let bitmap_count = if property_bytes != 0 && property_end > 64 {
            property_end.div_ceil(BITMAP_PAGES)
        } else {
            0
        };
        let end = property_end
            .checked_add(bitmap_count)
            .ok_or(CreateDatabaseError::PageCapacity)?;
        let length = (end as u64)
            .checked_mul(PAGE_BYTES as u64)
            .ok_or(CreateDatabaseError::PageCapacity)?;
        if length > crate::limits::MAX_READ_FILE_BYTES {
            return Err(InspectError::StructureLimit.into());
        }
        Ok(Self {
            definition: root..data,
            data,
            index,
            properties: property_start..property_end,
            bitmaps: property_end..end,
            end,
        })
    }

    pub fn owned_property_map_length(&self) -> usize {
        13_usize.max(1 + self.bitmaps.len() * size_of::<u32>())
    }
}

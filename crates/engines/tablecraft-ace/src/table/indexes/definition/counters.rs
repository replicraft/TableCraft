//! Original physical counter bytes; never inferred from current tree membership.
use crate::{
    FileFormat, InspectError, NativeReadSource, TableSchema,
    format::{
        bytes::u32_at,
        definition,
        layout::{COLUMN_BLOCK_OFFSET, INDEX_COUNT_OFFSET, INDEX_ENTRY_BYTES},
        page::PAGE_BYTES,
        reader::Reader,
    },
};
use std::ops::Range;

// IndexData.create in the pinned Jackcess source reads this field within each
// physical index's count descriptor. It is historical distinct-entry state,
// independent of the later 52-byte physical key/root/map descriptor.
const COUNTER_OFFSET: usize = size_of::<u32>();
const COUNTER_BYTES: usize = size_of::<u32>();

/// A requested physical slot, including slots with several logical aliases.
/// Extraction grants neither write admission nor permission to increment it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexHistoricalCounter {
    definition: u32,
    physical: u32,
    range: Range<usize>,
    value: u32,
}

impl IndexHistoricalCounter {
    /// Original first table-definition page; not a write handle.
    pub const fn definition_page(&self) -> u32 {
        self.definition
    }

    /// Physical index-data number, shared by logical aliases.
    pub const fn physical_index(&self) -> u32 {
        self.physical
    }

    /// Stored historical distinct-entry counter, not current live row count.
    /// Deletion and replacement need not decrement or recompute this value.
    pub const fn value(&self) -> u32 {
        self.value
    }

    /// Exact four-byte range in the decoded native image.
    /// This location does not grant mutation or encoded-file patch permission.
    pub fn byte_range(&self) -> Range<usize> {
        self.range.clone()
    }
}

/// Read one original counter without changing ordinary metadata-read budgets.
/// The full definition chain must complete before an absolute span is returned.
pub(in crate::table) fn read_counter<R: NativeReadSource>(
    reader: &mut Reader<'_, R>,
    schema: &TableSchema,
    physical: u32,
) -> Result<IndexHistoricalCounter, InspectError> {
    reader.tick()?;
    match reader.header.format() {
        FileFormat::Jet4
        | FileFormat::Ace2007
        | FileFormat::Ace2010
        | FileFormat::Ace2016
        | FileFormat::Ace2019 => {}
        FileFormat::Jet3 => return Err(InspectError::Unsupported),
    }
    let original = definition::read(reader, schema.definition)?;
    let stored_count = u32_at(&original, INDEX_COUNT_OFFSET)?;
    let count = usize::try_from(stored_count).map_err(|_| InspectError::StructureLimit)?;
    let physical_index = usize::try_from(physical).map_err(|_| InspectError::StructureLimit)?;
    if count != schema.layout.physical_indexes || physical_index >= count {
        return Err(InspectError::Malformed);
    }
    let relative = physical_index
        .checked_mul(INDEX_ENTRY_BYTES)
        .and_then(|offset| offset.checked_add(COLUMN_BLOCK_OFFSET))
        .and_then(|offset| offset.checked_add(COUNTER_OFFSET))
        .ok_or(InspectError::StructureLimit)?;
    let end = relative
        .checked_add(COUNTER_BYTES)
        .ok_or(InspectError::StructureLimit)?;
    // The current layout bounds physical slots to 256, so this count block is
    // wholly on the first definition page. Do not map a joined logical offset
    // into a guessed continuation-page location.
    if end > PAGE_BYTES {
        return Err(InspectError::Unsupported);
    }
    reader.work(COUNTER_BYTES)?;
    let value = u32_at(&original, relative)?;
    let start = (schema.definition as usize)
        .checked_mul(PAGE_BYTES)
        .and_then(|offset| offset.checked_add(relative))
        .ok_or(InspectError::StructureLimit)?;
    let end = start
        .checked_add(COUNTER_BYTES)
        .ok_or(InspectError::StructureLimit)?;
    reader.tick()?;
    Ok(IndexHistoricalCounter {
        definition: schema.definition,
        physical,
        range: start..end,
        value,
    })
}

#[cfg(all(test, feature = "ace16-write-preview"))]
mod tests;

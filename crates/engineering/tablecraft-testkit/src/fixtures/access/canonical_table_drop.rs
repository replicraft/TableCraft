//! Independent logical table deletion over closed canonical source inventories.
//!
//! The catalog row becomes a retained physical tombstone and its table's live
//! count decreases. This model does not retire or recycle native table storage.
pub mod current;

use super::{
    canonical_catalog_names, canonical_creator_rename, canonical_indexed_rename,
    canonical_ordinary_indexed::Case,
};

const PAGE_BYTES: usize = 4096;
const CATALOG_DEFINITION_PAGE: usize = 2;
const CATALOG_DATA_PAGE: usize = 4;
const LIVE_ROWS_OFFSET: usize = 16;
const SLOT_DIRECTORY_OFFSET: usize = 14;
const SLOT_BYTES: usize = size_of::<u16>();
const TOMBSTONE_BITS: u16 = 0xc000;

/// First-party BEFORE and exact expected AFTER, never a production writer result.
pub struct Pair {
    pub before: Vec<u8>,
    pub after: Vec<u8>,
    pub table: String,
    pub catalog_slot: u16,
}

/// Unindexed empty/populated tables with inline explicit Name AutoCorrect OFF.
pub fn unindexed(header: u8, name: &str, populated: bool) -> Pair {
    let original = canonical_catalog_names::rename_pair(header, name, name, populated);
    tombstone(original.before, name, 8)
}

/// Existing independently authored ordinary Int32 index families, unchanged.
pub fn indexed(header: u8, case: Case, name: &str) -> Pair {
    let original = canonical_indexed_rename::pair(header, case, name, name);
    tombstone(original.before, name, 8)
}

/// Creator sources whose explicit OFF record follows the user catalog row.
pub fn creator(original: &[u8]) -> Pair {
    let original = canonical_creator_rename::pair(original, "Items");
    tombstone(original.before, "Items", 7)
}

fn tombstone(before: Vec<u8>, table: &str, catalog_slot: u16) -> Pair {
    let directory = CATALOG_DATA_PAGE * PAGE_BYTES
        + SLOT_DIRECTORY_OFFSET
        + usize::from(catalog_slot) * SLOT_BYTES;
    let original_slot = u16::from_le_bytes(
        before[directory..directory + SLOT_BYTES]
            .try_into()
            .unwrap(),
    );
    assert_eq!(original_slot & TOMBSTONE_BITS, 0);

    let count_offset = CATALOG_DEFINITION_PAGE * PAGE_BYTES + LIVE_ROWS_OFFSET;
    let original_count = u32::from_le_bytes(
        before[count_offset..count_offset + size_of::<u32>()]
            .try_into()
            .unwrap(),
    );
    assert_eq!(original_count, 9);
    let mut after = before.clone();
    after[directory..directory + SLOT_BYTES]
        .copy_from_slice(&(original_slot | TOMBSTONE_BITS).to_le_bytes());
    after[count_offset..count_offset + size_of::<u32>()]
        .copy_from_slice(&(original_count - 1).to_le_bytes());

    Pair {
        before,
        after,
        table: table.into(),
        catalog_slot,
    }
}

#[cfg(test)]
mod tests;

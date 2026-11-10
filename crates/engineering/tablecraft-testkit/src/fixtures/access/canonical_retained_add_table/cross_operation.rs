//! Independent staged images for a retained cross-operation long-chain journey.
use super::{
    Target, deletes, desktop,
    rows::{self, Input, Profile},
};
use crate::fixtures::access::{
    canonical_created_number::mixed::after_off::deleted_field::populated::{
        Case, readdition::Target as Field,
    },
    canonical_table_drop::current as dropped,
    raw_forwarding_chain,
};

const PAGE: usize = 4096;

pub struct Source {
    pub initial: Vec<u8>,
    pub appended: Vec<u8>,
    pub inserted: Vec<u8>,
    pub updated: Vec<u8>,
    pub first_deleted: Vec<u8>,
    pub empty: Vec<u8>,
    pub reinserted: Vec<u8>,
    pub appended_again: Vec<u8>,
    pub catalog_root: usize,
    pub inserted_root: usize,
}

pub fn source() -> Source {
    let old = dropped::source(
        Case::MixedBinary,
        Field::ShortText,
        true,
        dropped::History::Inserted,
    );
    let retired = dropped::after(&old);
    let catalog_root = super::word(&retired, 4 * PAGE + 12);
    let first = desktop::append(&retired, "Table1", Target::GeneratedNumber);
    let first = rows::append(&first, 11, Profile::DesktopNumber, Input::Number(17));
    let user = raw_forwarding_chain::extend(&first, 12, 0, 20);
    let initial = raw_forwarding_chain::extend(&user, 4, catalog_root, 20);
    let appended = desktop::append(&initial, "Other", Target::GeneratedText);
    let inserted_root = super::word(&appended, 12 * PAGE + 12);
    let inserted = rows::append(&appended, 11, Profile::DesktopNumber, Input::Omitted);

    // Literal two-LONG row: count, generated ID, companion Score, presence mask.
    // This overwrite does not reuse the closed single-hop placement oracle.
    let original = deletes::record(&inserted, 11, 0);
    assert_eq!(original, &[2, 0, 1, 0, 0, 0, 17, 0, 0, 0, 3]);
    let mut record = original.to_vec();
    record[6..10].copy_from_slice(&123i32.to_le_bytes());
    let updated = raw_forwarding_chain::replace_terminal(&inserted, 12, 0, &record);
    let first_deleted = deletes::after(&updated, 11, &[0]);
    let empty = deletes::after(&first_deleted, 11, &[inserted_root]);
    let reinserted = rows::append(&empty, 11, Profile::DesktopNumber, Input::Omitted);
    let appended_again = desktop::append(&reinserted, "More", Target::GeneratedNumber);
    Source {
        initial,
        appended,
        inserted,
        updated,
        first_deleted,
        empty,
        reinserted,
        appended_again,
        catalog_root,
        inserted_root,
    }
}

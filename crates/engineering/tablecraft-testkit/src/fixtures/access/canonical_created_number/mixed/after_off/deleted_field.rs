//! Independent raw field deletion hypotheses with inert SDK reader observations.
//! This is not SDK DROP output or a native Access save/reopen qualification.
pub mod inserting;
pub mod populated;

#[cfg(test)]
mod tests;

use super::super::{SourceCase as MixedSource, source as mixed_source, storage};
use crate::fixtures::access::{canonical_database_options, raw_property_groups::without_group};

const PAGE: usize = 4096;
const DEFINITION_PAGE: usize = 7;
const NEXT_PAGE: usize = 4;
const DEFINITION_LENGTH: usize = 8;
const PHYSICAL_INDEXES: usize = 51;
const MAX_COLUMNS: usize = 41;
const ACTIVE_COLUMNS: usize = 45;
const COLUMN_BLOCK: usize = 63;
const INDEX_ENTRY_BYTES: usize = 12;
const COLUMN_BYTES: usize = 25;
const NATIVE_NUMBER: usize = 5;
const SHORT_BYTES: usize = size_of::<u16>();
const LONG_BYTES: usize = size_of::<u32>();

/// The two independently pinned empty sources in the raw DROP observation.
#[derive(Clone, Copy, Debug)]
pub enum SourceCase {
    /// Generated ID + Score + Score2, then explicit database OFF.
    NumberHistory,
    /// Generated ID + Label + AfterLabel + OtherLabel, then explicit OFF.
    MixedTextHistory,
}

/// Separate authored hypotheses; both have independent inert reader results.
#[derive(Clone, Copy, Debug)]
pub enum Variant {
    /// Remove active declaration/name, retaining the orphan live property group.
    DeclarationOnly,
    /// Append a new live body without that group; retain all old snapshots.
    AppendedGroupRemoved,
}

/// Build the original generated source through the existing independent owners.
pub fn source(case: SourceCase) -> Vec<u8> {
    match case {
        SourceCase::NumberHistory => canonical_database_options::append_explicit_off_to_owned_page(
            &mixed_source(MixedSource::TwoNumberHistory),
        ),
        SourceCase::MixedTextHistory => super::source(super::SourceCase::Mixed),
    }
}

fn word(bytes: &[u8], at: usize) -> usize {
    u16::from_le_bytes(bytes[at..at + SHORT_BYTES].try_into().unwrap()) as usize
}

fn long(bytes: &[u8], at: usize) -> usize {
    u32::from_le_bytes(bytes[at..at + LONG_BYTES].try_into().unwrap()) as usize
}

fn definition(before: &[u8], selected: &str) -> Vec<u8> {
    let page = &before[DEFINITION_PAGE * PAGE..(DEFINITION_PAGE + 1) * PAGE];
    assert_eq!(long(page, NEXT_PAGE), 0);
    assert_eq!(long(page, PHYSICAL_INDEXES), 1);
    let length = long(page, DEFINITION_LENGTH);
    assert_eq!(&page[length..], &vec![0; PAGE - length]);
    crate::fixtures::access::raw_schema_drop::definition(
        page,
        selected,
        crate::fixtures::access::raw_schema_drop::Slack::Zero,
    )
}

/// Remove one observed nonprimary scalar declaration from a pinned empty source.
/// Dictionary/group bytes and placement use first-party literal fixture owners;
/// this grants no authority for native schema writes or later edits.
pub fn remove_nonprimary(before: &[u8], selected: &str, variant: Variant) -> Vec<u8> {
    assert_eq!(before.len(), 11 * PAGE);
    assert!(selected.is_ascii());
    let rebuilt = definition(before, selected);
    let mut after = before.to_vec();
    after[DEFINITION_PAGE * PAGE..(DEFINITION_PAGE + 1) * PAGE].copy_from_slice(&rebuilt);
    match variant {
        Variant::DeclarationOnly => after,
        Variant::AppendedGroupRemoved => {
            let original = storage::selected_body(before);
            let body = without_group(&original, selected);
            storage::append(before, after, &body)
        }
    }
}

//! Independent raw field deletion hypotheses with inert SDK reader observations.
//! This is not SDK DROP output or a native Access save/reopen qualification.
pub mod inserting;

#[cfg(test)]
mod tests;

use super::super::{SourceCase as MixedSource, source as mixed_source, storage};
use crate::fixtures::access::{canonical_database_options, put16, put32};

const PAGE: usize = 4096;
const DEFINITION_PAGE: usize = 7;
const NEXT_PAGE: usize = 4;
const DEFINITION_LENGTH: usize = 8;
const FREE_BYTES: usize = 2;
const DEFINITION_FOOTER_BYTES: usize = 8;
const MAX_COLUMNS: usize = 41;
const ACTIVE_COLUMNS: usize = 45;
const PHYSICAL_INDEXES: usize = 51;
const COLUMN_BLOCK: usize = 63;
const INDEX_ENTRY_BYTES: usize = 12;
const COLUMN_BYTES: usize = 25;
const NATIVE_NUMBER: usize = 5;
const SHORT_BYTES: usize = size_of::<u16>();
const LONG_BYTES: usize = size_of::<u32>();
const NAME_LIST: usize = 128;
const COLUMN_GROUP: usize = 1;
const DICTIONARY_OFFSET: usize = 4;
const DICTIONARY_KIND: usize = 8;
const GROUP_KIND: usize = 4;
const GROUP_NAME_FRAME: usize = 6;
const GROUP_NAME_LENGTH: usize = 10;
const GROUP_NAME: usize = 12;

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

fn name(bytes: &[u8]) -> String {
    assert!(bytes.len().is_multiple_of(SHORT_BYTES));
    let units = bytes
        .as_chunks::<SHORT_BYTES>()
        .0
        .iter()
        .map(|word| u16::from_le_bytes(*word))
        .collect::<Vec<_>>();
    String::from_utf16(&units).unwrap()
}

fn definition(before: &[u8], selected: &str) -> Vec<u8> {
    let page = &before[DEFINITION_PAGE * PAGE..(DEFINITION_PAGE + 1) * PAGE];
    assert_eq!(long(page, NEXT_PAGE), 0);
    assert_eq!(long(page, PHYSICAL_INDEXES), 1);
    let length = long(page, DEFINITION_LENGTH);
    assert_eq!(&page[length..], &vec![0; PAGE - length]);
    let count = word(page, ACTIVE_COLUMNS);
    let entries = COLUMN_BLOCK + long(page, PHYSICAL_INDEXES) * INDEX_ENTRY_BYTES;
    let mut position = entries + count * COLUMN_BYTES;
    let mut kept_entries = Vec::new();
    let mut kept_names = Vec::new();
    let mut removed = 0;
    for index in 0..count {
        let entry = &page[entries + index * COLUMN_BYTES..entries + (index + 1) * COLUMN_BYTES];
        let size = word(page, position);
        let end = position + SHORT_BYTES + size;
        let label = name(&page[position + SHORT_BYTES..end]);
        if label == selected {
            assert_ne!(word(entry, NATIVE_NUMBER), 0);
            removed += 1;
        } else {
            kept_entries.extend_from_slice(entry);
            kept_names.extend_from_slice(&page[position..end]);
        }
        position = end;
    }
    assert_eq!(removed, 1);
    let mut rebuilt = page[..entries].to_vec();
    rebuilt.extend(kept_entries);
    rebuilt.extend(kept_names);
    rebuilt.extend_from_slice(&page[position..length]);
    put16(&mut rebuilt, ACTIVE_COLUMNS, (count - 1) as u16);
    let new_length = rebuilt.len();
    put32(&mut rebuilt, DEFINITION_LENGTH, new_length as u32);
    put16(
        &mut rebuilt,
        FREE_BYTES,
        (PAGE - new_length - DEFINITION_FOOTER_BYTES) as u16,
    );
    rebuilt.resize(PAGE, 0);
    assert_eq!(
        &rebuilt[MAX_COLUMNS..ACTIVE_COLUMNS],
        &page[MAX_COLUMNS..ACTIVE_COLUMNS]
    );
    rebuilt
}

pub(super) fn without_group(body: &[u8], selected: &str) -> Vec<u8> {
    assert_eq!(&body[..DICTIONARY_OFFSET], b"MR2\0");
    assert_eq!(word(body, DICTIONARY_KIND), NAME_LIST);
    let mut position = DICTIONARY_OFFSET + long(body, DICTIONARY_OFFSET);
    let mut result = body[..position].to_vec();
    let mut removed = 0;
    while position < body.len() {
        let length = long(body, position);
        let name_bytes = word(body, position + GROUP_NAME_LENGTH);
        assert_eq!(
            long(body, position + GROUP_NAME_FRAME),
            name_bytes + LONG_BYTES + SHORT_BYTES
        );
        assert!(GROUP_NAME + name_bytes <= length);
        let label = name(&body[position + GROUP_NAME..position + GROUP_NAME + name_bytes]);
        if word(body, position + GROUP_KIND) == COLUMN_GROUP && label == selected {
            removed += 1;
        } else {
            result.extend_from_slice(&body[position..position + length]);
        }
        position += length;
    }
    assert_eq!(position, body.len());
    assert_eq!(removed, 1);
    result
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

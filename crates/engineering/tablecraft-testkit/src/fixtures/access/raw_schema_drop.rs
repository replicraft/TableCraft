//! Independent native field removal preserves high-water marks and row bytes.
use super::{put16, put32};

const PAGE: usize = 4096;
const ACTIVE_COLUMNS: usize = 45;
const MAX_COLUMNS: usize = 41;
const DEFINITION_LENGTH: usize = 8;
const FREE_BYTES: usize = 2;
const DEFINITION_FOOTER_BYTES: usize = 8;
const COLUMN_BLOCK: usize = 63;
const PHYSICAL_INDEXES: usize = 51;
const INDEX_ENTRY_BYTES: usize = 12;
const COLUMN_BYTES: usize = 25;
const NATIVE_NUMBER: usize = 5;
const SHORT_BYTES: usize = 2;

fn word(bytes: &[u8], at: usize) -> usize {
    u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()) as usize
}

fn long(bytes: &[u8], at: usize) -> usize {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize
}

fn name(bytes: &[u8]) -> String {
    String::from_utf16(
        &bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_le_bytes(*pair))
            .collect::<Vec<_>>(),
    )
    .unwrap()
}

#[derive(Clone, Copy)]
pub(in crate::fixtures::access) enum Slack {
    Zero,
    Preserve,
}

pub(in crate::fixtures::access) fn definition(
    page: &[u8],
    selected: &str,
    slack: Slack,
) -> Vec<u8> {
    let length = long(page, DEFINITION_LENGTH);
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
    if matches!(slack, Slack::Preserve) {
        rebuilt[length..].copy_from_slice(&page[length..]);
    }
    assert_eq!(
        &rebuilt[MAX_COLUMNS..ACTIVE_COLUMNS],
        &page[MAX_COLUMNS..ACTIVE_COLUMNS]
    );
    rebuilt
}

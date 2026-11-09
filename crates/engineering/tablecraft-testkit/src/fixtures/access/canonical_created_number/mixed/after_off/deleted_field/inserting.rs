//! Independent empty sparse LONG sources and literal first-row outcomes.
use super::{
    ACTIVE_COLUMNS, COLUMN_BLOCK, COLUMN_BYTES, DEFINITION_PAGE, INDEX_ENTRY_BYTES, MAX_COLUMNS,
    NATIVE_NUMBER, PAGE, PHYSICAL_INDEXES, definition, storage, without_group, word,
};
use crate::fixtures::access::{
    canonical_created_number::{Source, repeated},
    canonical_database_options,
    canonical_initial_table_creation::inserts::first_row_image,
    put16, put32,
};

const FIXED_OFFSET: usize = 21;
const LONG_WIDTH: usize = 4;
const HEADER_BYTES: usize = 2;
const MASK_BITS: usize = 8;

/// Build one raw current snapshot after removing multiple distinct Number fields.
/// Retired MR2 rows remain exact, and no engine code supplies any source bytes.
pub fn number_source(number_fields: usize, removed: &[usize]) -> Vec<u8> {
    assert!((1..=9).contains(&number_fields));
    let mut before = repeated::source(Source::IdOnly);
    for number in 2..=number_fields {
        before = repeated::append(&before, &format!("N{number}"));
    }
    let before = canonical_database_options::append_explicit_off_to_owned_page(&before);
    assert_eq!(before.len(), 11 * PAGE);
    let mut after = before.clone();
    let mut body = storage::selected_body(&before);
    for &number in removed {
        assert!((1..=number_fields).contains(&number));
        let selected = if number == 1 {
            "Score".to_owned()
        } else {
            format!("N{number}")
        };
        let page = definition(&after, &selected);
        after[DEFINITION_PAGE * PAGE..(DEFINITION_PAGE + 1) * PAGE].copy_from_slice(&page);
        body = without_group(&body, &selected);
    }
    assert!(!removed.is_empty());
    storage::append(&before, after, &body)
}

/// Explicit values are keyed by actual native number, not a compacted position.
/// The raw missing native slots have zero padding and clear presence bits.
pub fn first_number_row(before: &[u8], values: &[(usize, Option<i32>)]) -> Vec<u8> {
    let page = &before[DEFINITION_PAGE * PAGE..(DEFINITION_PAGE + 1) * PAGE];
    let count = word(page, ACTIVE_COLUMNS);
    let maximum = word(page, MAX_COLUMNS);
    let start = COLUMN_BLOCK + INDEX_ENTRY_BYTES * word(page, PHYSICAL_INDEXES);
    assert_eq!(values.len(), count);
    let mut end = HEADER_BYTES;
    for index in 0..count {
        let entry = start + index * COLUMN_BYTES;
        end = end.max(HEADER_BYTES + word(page, entry + FIXED_OFFSET) + LONG_WIDTH);
    }
    let mut record = vec![0; end + maximum.div_ceil(MASK_BITS)];
    put16(&mut record, 0, maximum as u16);
    for index in 0..count {
        let entry = start + index * COLUMN_BYTES;
        let number = word(page, entry + NATIVE_NUMBER);
        let matches = values
            .iter()
            .filter(|(native, _)| *native == number)
            .collect::<Vec<_>>();
        assert_eq!(matches.len(), 1);
        if let Some(value) = matches[0].1 {
            let offset = HEADER_BYTES + word(page, entry + FIXED_OFFSET);
            put32(&mut record, offset, value as u32);
            record[end + number / MASK_BITS] |= 1 << (number % MASK_BITS);
        }
    }
    assert_eq!(word(page, PHYSICAL_INDEXES), 1);
    assert_eq!(word(&before[8 * PAGE..9 * PAGE], 12), 0);
    // A native record keeps high-water count, not just values.len().
    assert_eq!(word(&record, 0), maximum);
    first_row_image(before, record)
}

/// A separate raw identity case puts generation at native2, not vector position0.
/// Stored IDs4/12 intentionally differ from the native mask/key numbers2/9.
pub fn shifted_pair(before: &[u8]) -> Vec<u8> {
    const STORED_ID: usize = 9;
    const INDEX_KEY: usize = 4;
    let mut after = before.to_vec();
    let page = &mut after[DEFINITION_PAGE * PAGE..(DEFINITION_PAGE + 1) * PAGE];
    assert_eq!(word(page, ACTIVE_COLUMNS), 2);
    let entries = COLUMN_BLOCK + INDEX_ENTRY_BYTES * word(page, PHYSICAL_INDEXES);
    assert_eq!(word(page, entries + NATIVE_NUMBER), 0);
    assert_eq!(word(page, entries + COLUMN_BYTES + NATIVE_NUMBER), 2);
    let mut tail = entries + 2 * COLUMN_BYTES;
    for _ in 0..2 {
        tail += HEADER_BYTES + word(page, tail);
    }
    assert_eq!(word(page, tail + INDEX_KEY), 0);
    put16(page, entries + NATIVE_NUMBER, 2);
    put16(page, entries + STORED_ID, 4);
    put16(page, entries + COLUMN_BYTES + NATIVE_NUMBER, 9);
    put16(page, entries + COLUMN_BYTES + STORED_ID, 12);
    put16(page, MAX_COLUMNS, 10);
    put16(page, tail + INDEX_KEY, 2);
    after
}

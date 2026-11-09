//! Literal logical deletion of one generated-ID/two-LONG initial-table row.
use crate::fixtures::access::{put16, put32};

const PAGE: usize = 4096;
const DEFINITION: usize = 7;
const DATA: usize = 8;
const INDEX: usize = 9;
const FREE: usize = 2;
const OWNER: usize = 4;
const ROW_COUNT: usize = 12;
const DIRECTORY: usize = 14;
const LIVE_ROWS: usize = 16;
const LAST_ISSUED: usize = 20;
const UNIQUE_HISTORY: usize = 67;
const BITMAP: usize = 27;
const ENTRIES: usize = 480;
const DELETED: u16 = 0xc000;
const RECORD_BYTES: usize = 11;
const ID_ONE_KEY: [u8; 9] = [127, 128, 0, 0, 1, 0, 0, 8, 0];

fn word(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap())
}

fn long(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}

/// Delete the only live ID1 record, retaining its companion value and every
/// inactive byte. No row encoder, index encoder or native planner participates.
/// Name/property/history bytes are opaque, so fitted rename tails survive.
pub fn only_long_row(before: &[u8]) -> Vec<u8> {
    assert_eq!(before.len(), 11 * PAGE);
    let definition = DEFINITION * PAGE;
    assert_eq!(word(before, definition + 41), 2);
    assert_eq!(word(before, definition + 43), 0);
    assert_eq!(word(before, definition + 45), 2);
    assert_eq!(long(before, definition + 47), 1);
    assert_eq!(long(before, definition + 51), 1);
    assert_eq!(long(before, definition + LIVE_ROWS), 1);
    assert_eq!(long(before, definition + LAST_ISSUED), 1);
    assert_eq!(long(before, definition + UNIQUE_HISTORY), 1);

    let data = DATA * PAGE;
    assert_eq!(&before[data..data + 2], &[1, 1]);
    assert_eq!(long(before, data + OWNER), DEFINITION as u32);
    assert_eq!(word(before, data + ROW_COUNT), 1);
    let original_word = word(before, data + DIRECTORY);
    assert_eq!(usize::from(original_word), PAGE - RECORD_BYTES);
    let record = &before[data + usize::from(original_word)..data + PAGE];
    assert_eq!(&record[..6], &[2, 0, 1, 0, 0, 0]);
    assert_eq!(record[10], 3);

    let index = INDEX * PAGE;
    assert_eq!(&before[index..index + 2], &[4, 1]);
    assert_eq!(long(before, index + OWNER), DEFINITION as u32);
    assert_eq!(
        word(before, index + FREE) as usize,
        PAGE - ENTRIES - ID_ONE_KEY.len()
    );
    assert_eq!(&before[index + 8..index + BITMAP], &[0; 19]);
    let mut original_bitmap = [0; ENTRIES - BITMAP];
    original_bitmap[ID_ONE_KEY.len() / 8] = 1 << (ID_ONE_KEY.len() % 8);
    assert_eq!(&before[index + BITMAP..index + ENTRIES], &original_bitmap);
    assert_eq!(
        &before[index + ENTRIES..index + ENTRIES + ID_ONE_KEY.len()],
        &ID_ONE_KEY
    );

    let mut after = before.to_vec();
    put16(&mut after, data + DIRECTORY, original_word | DELETED);
    put32(&mut after, definition + LIVE_ROWS, 0);
    put16(&mut after, index + FREE, (PAGE - ENTRIES) as u16);
    after[index + BITMAP..index + ENTRIES].fill(0);
    after
}

#[cfg(all(test, feature = "access-support"))]
mod tests;

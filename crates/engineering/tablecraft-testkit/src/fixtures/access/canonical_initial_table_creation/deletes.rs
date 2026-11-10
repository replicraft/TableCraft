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
const COLUMN_BLOCK: usize = 75;
const COLUMN_BYTES: usize = 25;
const NATIVE_NUMBER: usize = 5;
const STORED_ID: usize = 9;
const FLAGS: usize = 15;
const FIXED_OFFSET: usize = 21;
const WIDTH: usize = 23;
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

    for (number, flags) in [(0, 7), (1, 3)] {
        let column = definition + COLUMN_BLOCK + number * COLUMN_BYTES;
        assert_eq!(before[column], 4);
        assert_eq!(word(before, column + NATIVE_NUMBER) as usize, number);
        assert_eq!(word(before, column + STORED_ID) as usize, number);
        assert_eq!(before[column + FLAGS], flags);
        assert_eq!(word(before, column + FIXED_OFFSET) as usize, number * 4);
        assert_eq!(word(before, column + WIDTH), 4);
    }

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

/// Delete one original physical slot from a dense ID1..N / nullable LONG batch.
/// The retained key prefix follows the independent indexed-delete model; newly
/// inactive key bytes and every physical record remain exactly as supplied.
/// This helper deliberately refuses already deleted or reordered memberships.
pub fn long_row(before: &[u8], row: usize) -> Vec<u8> {
    assert_eq!(before.len(), 11 * PAGE);
    let definition = DEFINITION * PAGE;
    assert_eq!(
        &before[definition + 41..definition + 47],
        &[2, 0, 0, 0, 2, 0]
    );
    assert_eq!(long(before, definition + 47), 1);
    assert_eq!(long(before, definition + 51), 1);
    for (number, flags) in [(0, 7), (1, 3)] {
        let column = definition + COLUMN_BLOCK + number * COLUMN_BYTES;
        assert_eq!(before[column], 4);
        assert_eq!(word(before, column + NATIVE_NUMBER) as usize, number);
        assert_eq!(word(before, column + STORED_ID) as usize, number);
        assert_eq!(before[column + FLAGS], flags);
        assert_eq!(word(before, column + FIXED_OFFSET) as usize, number * 4);
        assert_eq!(word(before, column + WIDTH), 4);
    }

    let count = long(before, definition + LIVE_ROWS) as usize;
    assert!((1..=255).contains(&count));
    assert!(row < count);
    assert_eq!(long(before, definition + LAST_ISSUED) as usize, count);
    assert_eq!(long(before, definition + UNIQUE_HISTORY) as usize, count);
    let data = DATA * PAGE;
    assert_eq!(&before[data..data + 2], &[1, 1]);
    assert_eq!(long(before, data + OWNER), DEFINITION as u32);
    assert_eq!(word(before, data + ROW_COUNT) as usize, count);
    assert_eq!(
        word(before, data + FREE) as usize,
        PAGE - DIRECTORY - count * (2 + RECORD_BYTES)
    );

    let index = INDEX * PAGE;
    assert_eq!(&before[index..index + 2], &[4, 1]);
    assert_eq!(long(before, index + OWNER), DEFINITION as u32);
    assert_eq!(&before[index + 8..index + BITMAP], &[0; 19]);
    assert_eq!(
        word(before, index + FREE) as usize,
        PAGE - ENTRIES - count * ID_ONE_KEY.len()
    );
    let mut bitmap = [0_u8; ENTRIES - BITMAP];
    let mut retained = Vec::new();
    for slot in 0..count {
        let start = PAGE - (slot + 1) * RECORD_BYTES;
        assert_eq!(word(before, data + DIRECTORY + slot * 2) as usize, start);
        let record = &before[data + start..data + start + RECORD_BYTES];
        assert_eq!(&record[..2], &[2, 0]);
        assert_eq!(long(record, 2) as usize, slot + 1);
        assert!([1, 3].contains(&record[10]));
        if record[10] == 1 {
            assert_eq!(&record[6..10], &[0; 4]);
        }

        let mut key = [0_u8; 9];
        key[0] = 127;
        key[1..5].copy_from_slice(&((slot as u32 + 1) ^ 0x8000_0000).to_be_bytes());
        key[5..].copy_from_slice(&((DATA as u32) << 8 | slot as u32).to_be_bytes());
        let at = index + ENTRIES + slot * key.len();
        assert_eq!(&before[at..at + key.len()], &key);
        let end = (slot + 1) * key.len();
        bitmap[end / 8] |= 1 << (end % 8);
        if slot != row {
            retained.extend_from_slice(&key);
        }
    }
    assert_eq!(&before[index + BITMAP..index + ENTRIES], &bitmap);

    let mut after = before.to_vec();
    let directory = data + DIRECTORY + row * 2;
    put16(&mut after, directory, word(before, directory) | DELETED);
    put32(&mut after, definition + LIVE_ROWS, (count - 1) as u32);
    put16(
        &mut after,
        index + FREE,
        (PAGE - ENTRIES - retained.len()) as u16,
    );
    after[index + BITMAP..index + ENTRIES].fill(0);
    for end in (ID_ONE_KEY.len()..=retained.len()).step_by(ID_ONE_KEY.len()) {
        after[index + BITMAP + end / 8] |= 1 << (end % 8);
    }
    after[index + ENTRIES..index + ENTRIES + retained.len()].copy_from_slice(&retained);
    after
}

#[cfg(all(test, feature = "access-support"))]
mod tests;

//! Physical append slots, generated history and live index membership are distinct.
use crate::fixtures::access::{put16, put32};

const PAGE: usize = 4096;
const DEFINITION: usize = 7 * PAGE;
const DATA: usize = 8 * PAGE;
const INDEX: usize = 9 * PAGE;
const DIRECTORY: usize = 14;
const ENTRIES: usize = 480;
const BITMAP: usize = 27;
const KEY_BYTES: usize = 9;
const LIVE: usize = 16;
const LAST: usize = 20;
const UNIQUE_HISTORY: usize = 67;

fn word(bytes: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()))
}

fn long(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}

pub(super) fn next_id(before: &[u8]) -> i32 {
    i32::try_from(long(before, DEFINITION + LAST))
        .unwrap()
        .checked_add(1)
        .unwrap()
}

pub(super) fn append(before: &[u8], id: i32, record: &[u8]) -> Vec<u8> {
    assert_eq!(before.len(), 11 * PAGE);
    let physical = word(before, DATA + 12);
    let live = long(before, DEFINITION + LIVE);
    let history = long(before, DEFINITION + UNIQUE_HISTORY);
    let frontier = word(before, DATA + DIRECTORY + (physical - 1) * 2) & 0x1fff;
    assert_eq!(word(before, DATA + 2), frontier - DIRECTORY - physical * 2);
    let start = frontier.checked_sub(record.len()).unwrap();
    assert!(start >= DIRECTORY + (physical + 1) * 2);
    assert!(u8::try_from(physical).is_ok());
    assert_eq!(word(before, INDEX + 24), 0);
    assert_eq!(
        word(before, INDEX + 2),
        PAGE - ENTRIES - live as usize * KEY_BYTES
    );
    let mut after = before.to_vec();
    after[DATA + start..DATA + frontier].copy_from_slice(record);
    put16(&mut after, DATA + DIRECTORY + physical * 2, start as u16);
    put16(&mut after, DATA + 12, (physical + 1) as u16);
    put16(
        &mut after,
        DATA + 2,
        (start - DIRECTORY - (physical + 1) * 2) as u16,
    );
    put32(&mut after, DEFINITION + LIVE, live + 1);
    put32(&mut after, DEFINITION + LAST, id as u32);
    put32(&mut after, DEFINITION + UNIQUE_HISTORY, history + 1);

    let entry = INDEX + ENTRIES + live as usize * KEY_BYTES;
    after[entry] = 127;
    after[entry + 1..entry + 5].copy_from_slice(&((id as u32) ^ 0x8000_0000).to_be_bytes());
    after[entry + 5..entry + 9].copy_from_slice(&((8_u32 << 8) | physical as u32).to_be_bytes());
    let end = (live as usize + 1) * KEY_BYTES;
    after[INDEX + BITMAP + end / 8] |= 1 << (end % 8);
    put16(&mut after, INDEX + 2, (PAGE - ENTRIES - end) as u16);
    after
}

//! Physical append slots, generated history and live index membership are distinct.
use crate::fixtures::access::{put16, put32};

const PAGE: usize = 4096;
const DIRECTORY: usize = 14;
const ENTRIES: usize = 480;
const BITMAP: usize = 27;
const KEY_BYTES: usize = 9;
const LIVE: usize = 16;
const LAST: usize = 20;
const UNIQUE_HISTORY: usize = 67;

#[derive(Clone, Copy)]
pub(in crate::fixtures::access) struct Pages {
    pub definition: usize,
    pub data: usize,
    pub index: usize,
}

fn word(bytes: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()))
}

fn long(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}

pub(in crate::fixtures::access) fn next_id(before: &[u8], pages: Pages) -> i32 {
    let definition = pages.definition * PAGE;
    i32::try_from(long(before, definition + LAST))
        .unwrap()
        .checked_add(1)
        .unwrap()
}

pub(in crate::fixtures::access) fn append(
    before: &[u8],
    pages: Pages,
    id: i32,
    record: &[u8],
) -> Vec<u8> {
    let definition = pages.definition * PAGE;
    let data = pages.data * PAGE;
    let index = pages.index * PAGE;
    let physical = word(before, data + 12);
    let live = long(before, definition + LIVE);
    let history = long(before, definition + UNIQUE_HISTORY);
    let frontier = if physical == 0 {
        PAGE
    } else {
        word(before, data + DIRECTORY + (physical - 1) * 2) & 0x1fff
    };
    assert_eq!(word(before, data + 2), frontier - DIRECTORY - physical * 2);
    let start = frontier.checked_sub(record.len()).unwrap();
    assert!(start >= DIRECTORY + (physical + 1) * 2);
    assert!(u8::try_from(physical).is_ok());
    assert_eq!(word(before, index + 24), 0);
    assert_eq!(
        word(before, index + 2),
        PAGE - ENTRIES - live as usize * KEY_BYTES
    );
    let mut after = before.to_vec();
    after[data + start..data + frontier].copy_from_slice(record);
    put16(&mut after, data + DIRECTORY + physical * 2, start as u16);
    put16(&mut after, data + 12, (physical + 1) as u16);
    put16(
        &mut after,
        data + 2,
        (start - DIRECTORY - (physical + 1) * 2) as u16,
    );
    put32(&mut after, definition + LIVE, live + 1);
    put32(&mut after, definition + LAST, id as u32);
    put32(&mut after, definition + UNIQUE_HISTORY, history + 1);

    let entry = index + ENTRIES + live as usize * KEY_BYTES;
    after[entry] = 127;
    after[entry + 1..entry + 5].copy_from_slice(&((id as u32) ^ 0x8000_0000).to_be_bytes());
    after[entry + 5..entry + 9]
        .copy_from_slice(&(u32::try_from((pages.data << 8) | physical).unwrap()).to_be_bytes());
    let end = (live as usize + 1) * KEY_BYTES;
    after[index + BITMAP + end / 8] |= 1 << (end % 8);
    put16(&mut after, index + 2, (PAGE - ENTRIES - end) as u16);
    after
}

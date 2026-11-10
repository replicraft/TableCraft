//! Independent fixed-row materialization and logical-root forwarding oracle.
use super::{COUNTERS, DATA, DEFINITION, INDEX, PAGE, SourceCase, source};
use crate::fixtures::access::{put16, put32};

/// Append another nullable zero-default LONG while keeping all actual records.
/// Old fields remain physically absent; only their declaration history grows.
pub fn append_number(before: &[u8], name: &str) -> Vec<u8> {
    let mut empty = before.to_vec();
    for offset in COUNTERS {
        put32(&mut empty, DEFINITION * PAGE + offset, 0);
    }
    let original_empty = source(SourceCase::Number);
    empty[DATA * PAGE..(INDEX + 1) * PAGE]
        .copy_from_slice(&original_empty[DATA * PAGE..(INDEX + 1) * PAGE]);
    let mut after = super::super::append_number(&empty, name);
    for offset in COUNTERS {
        let at = DEFINITION * PAGE + offset;
        after[at..at + 4].copy_from_slice(&before[at..at + 4]);
    }
    after[DATA * PAGE..(INDEX + 1) * PAGE]
        .copy_from_slice(&before[DATA * PAGE..(INDEX + 1) * PAGE]);
    after
}

/// Set one genuinely absent Number, append a hidden complete fixed record and
/// forward its original logical root. ID index entries and row counters stay put.
pub fn after(before: &[u8], slot: usize, column: usize, value: i32) -> Vec<u8> {
    const DIRECTORY: usize = 14;
    const ROW_COUNT: usize = 12;
    const FIELD_START: usize = 75;
    const FIELD_BYTES: usize = 25;

    fn word(bytes: &[u8], at: usize) -> usize {
        usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()))
    }

    assert_eq!(before.len(), 11 * PAGE);
    let definition = DEFINITION * PAGE;
    let fields = word(before, definition + 41);
    assert!((3..16).contains(&fields));
    for number in 0..fields {
        let at = definition + FIELD_START + number * FIELD_BYTES;
        assert_eq!(before[at], 4);
        assert_eq!(word(before, at + 5), number);
        assert_eq!(word(before, at + 21), number * 4);
        assert_eq!(word(before, at + 23), 4);
    }
    let data = DATA * PAGE;
    let count = word(before, data + ROW_COUNT);
    assert!(slot < count && count < 255);
    let original = word(before, data + DIRECTORY + slot * 2);
    assert_eq!(original & !0x1fff, 0);
    let end = if slot == 0 {
        PAGE
    } else {
        word(before, data + DIRECTORY + (slot - 1) * 2) & 0x1fff
    };
    let old_fields = word(before, data + original);
    assert!(column >= old_fields && column < fields);
    let old_mask = old_fields.div_ceil(8);
    assert_eq!(end - original, 2 + old_fields * 4 + old_mask);
    let old = &before[data + original..data + end];
    let mut record = vec![0; 2 + fields * 4 + fields.div_ceil(8)];
    record[..2].copy_from_slice(&(fields as u16).to_le_bytes());
    record[2..2 + old_fields * 4].copy_from_slice(&old[2..2 + old_fields * 4]);
    let mask = 2 + fields * 4;
    record[mask..mask + old_mask].copy_from_slice(&old[old.len() - old_mask..]);
    for number in old_fields..fields {
        record[mask + number / 8] &= !(1 << (number % 8));
    }
    record[2 + column * 4..2 + (column + 1) * 4].copy_from_slice(&value.to_le_bytes());
    record[mask + column / 8] |= 1 << (column % 8);

    place_record(before, slot, &record)
}

/// Independent direct-root placement shared by fixed and mixed record oracles.
/// The caller supplies the independently constructed complete new row bytes.
pub(in crate::fixtures::access::canonical_created_number::mixed::after_off) fn place_record(
    before: &[u8],
    slot: usize,
    record: &[u8],
) -> Vec<u8> {
    const DIRECTORY: usize = 14;
    const ROW_COUNT: usize = 12;
    const FREE: usize = 2;

    fn word(bytes: &[u8], at: usize) -> usize {
        usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()))
    }

    assert_eq!(before.len(), 11 * PAGE);
    let data = DATA * PAGE;
    let count = word(before, data + ROW_COUNT);
    assert!(slot < count && count < 255);
    let original = word(before, data + DIRECTORY + slot * 2);
    assert_eq!(original & !0x1fff, 0);
    let end = if slot == 0 {
        PAGE
    } else {
        word(before, data + DIRECTORY + (slot - 1) * 2) & 0x1fff
    };
    assert!(record.len() > end - original);
    let frontier = word(before, data + DIRECTORY + (count - 1) * 2) & 0x1fff;
    let target = frontier.checked_sub(record.len()).unwrap();
    let free = word(before, data + FREE);
    assert_eq!(free, frontier - DIRECTORY - count * 2);
    assert!(target >= DIRECTORY + (count + 1) * 2);
    let mut after = before.to_vec();
    after[data + target..data + frontier].copy_from_slice(&record);
    put16(
        &mut after,
        data + DIRECTORY + count * 2,
        target as u16 | 0x8000,
    );
    put16(&mut after, data + ROW_COUNT, (count + 1) as u16);
    put16(&mut after, data + FREE, (free - record.len() - 2) as u16);
    after[data + original..data + end].fill(0);
    put32(
        &mut after,
        data + original,
        ((DATA as u32) << 8) | count as u32,
    );
    put16(
        &mut after,
        data + DIRECTORY + slot * 2,
        original as u16 | 0x4000,
    );
    after
}

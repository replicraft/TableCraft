//! Independent metadata-only Number ADD over the canonical existing row family.
pub mod materialization;
use super::{SourceCase, source};
use crate::fixtures::access::{canonical_initial_table_creation::inserts::long_rows, put16, put32};

const PAGE: usize = 4096;
const DEFINITION: usize = 7;
const DATA: usize = 8;
const INDEX: usize = 9;
const COUNTERS: [usize; 3] = [16, 20, 67];

/// Dense generated IDs and nullable old Number payloads precede the schema ADD.
/// Their original two-field records remain byte-exact: the new field is absent
/// from those records, which is distinct from a future omitted default of zero.
pub fn pair(name: &str, values: &[Option<i32>]) -> [Vec<u8>; 2] {
    let before = long_rows(&source(SourceCase::Number), values);
    let after = append_number(&before, name);
    [before, after]
}

/// Apply the existing independently authored empty metadata transformation while
/// retaining actual row/index pages and all generation/cardinality counters.
/// Only the canonical source metadata is admitted by this raw fixture helper.
pub fn append_number(before: &[u8], name: &str) -> Vec<u8> {
    let empty = source(SourceCase::Number);
    assert_eq!(before.len(), empty.len());
    let mut metadata = before.to_vec();
    for offset in COUNTERS {
        put32(&mut metadata, DEFINITION * PAGE + offset, 0);
    }
    metadata[DATA * PAGE..(INDEX + 1) * PAGE]
        .copy_from_slice(&empty[DATA * PAGE..(INDEX + 1) * PAGE]);
    assert_eq!(metadata, empty);

    let mut after = super::append_number(&empty, name);
    for offset in COUNTERS {
        let at = DEFINITION * PAGE + offset;
        after[at..at + 4].copy_from_slice(&before[at..at + 4]);
    }
    after[DATA * PAGE..(INDEX + 1) * PAGE]
        .copy_from_slice(&before[DATA * PAGE..(INDEX + 1) * PAGE]);
    after
}

/// Append one current three-LONG record after the historical two-field rows.
/// Values describe literal stored outcomes, never a default-evaluation policy.
/// Original physical slots and every older record byte remain unchanged.
pub fn append_row(before: &[u8], score: Option<i32>, bonus: Option<i32>) -> Vec<u8> {
    const DIRECTORY: usize = 14;
    const ROW_COUNT: usize = 12;
    const FREE: usize = 2;
    const ENTRIES: usize = 480;
    const BITMAP: usize = 27;
    const KEY_BYTES: usize = 9;
    const COLUMN_START: usize = 75;
    const COLUMN_BYTES: usize = 25;
    const RECORD_BYTES: usize = 15;

    fn word(bytes: &[u8], at: usize) -> usize {
        usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()))
    }

    assert_eq!(before.len(), 11 * PAGE);
    let definition = DEFINITION * PAGE;
    assert_eq!(
        &before[definition + 41..definition + 47],
        &[3, 0, 0, 0, 3, 0]
    );
    for number in 0..3 {
        let column = definition + COLUMN_START + number * COLUMN_BYTES;
        assert_eq!(before[column], 4);
        assert_eq!(word(before, column + 5), number);
        assert_eq!(word(before, column + 21), number * 4);
        assert_eq!(word(before, column + 23), 4);
    }
    let data = DATA * PAGE;
    let count = word(before, data + ROW_COUNT);
    assert!((1..255).contains(&count));
    for offset in COUNTERS {
        assert_eq!(
            u32::from_le_bytes(
                before[definition + offset..definition + offset + 4]
                    .try_into()
                    .unwrap()
            ) as usize,
            count
        );
    }
    let mut end = PAGE;
    for slot in 0..count {
        let start = word(before, data + DIRECTORY + slot * 2);
        let fields = word(before, data + start);
        assert!([2, 3].contains(&fields));
        assert_eq!(end - start, 3 + fields * 4);
        assert_eq!(
            u32::from_le_bytes(
                before[data + start + 2..data + start + 6]
                    .try_into()
                    .unwrap()
            ) as usize,
            slot + 1
        );
        end = start;
    }
    assert_eq!(word(before, data + FREE), end - DIRECTORY - count * 2);
    let index = INDEX * PAGE;
    assert_eq!(
        word(before, index + FREE),
        PAGE - ENTRIES - count * KEY_BYTES
    );
    let start = end - RECORD_BYTES;
    assert!(start >= DIRECTORY + (count + 1) * 2);
    let id = (count + 1) as u32;
    let mut record = vec![3, 0];
    record.extend(id.to_le_bytes());
    record.extend(score.unwrap_or(0).to_le_bytes());
    record.extend(bonus.unwrap_or(0).to_le_bytes());
    let score_present = if score.is_some() { 2 } else { 0 };
    let bonus_present = if bonus.is_some() { 4 } else { 0 };
    record.push(1 | score_present | bonus_present);

    let mut after = before.to_vec();
    after[data + start..data + end].copy_from_slice(&record);
    put16(&mut after, data + DIRECTORY + count * 2, start as u16);
    put16(&mut after, data + ROW_COUNT, id as u16);
    put16(
        &mut after,
        data + FREE,
        (start - DIRECTORY - (count + 1) * 2) as u16,
    );
    for offset in COUNTERS {
        put32(&mut after, definition + offset, id);
    }
    let key = index + ENTRIES + count * KEY_BYTES;
    after[key] = 127;
    after[key + 1..key + 5].copy_from_slice(&(id ^ 0x8000_0000).to_be_bytes());
    after[key + 5..key + 9].copy_from_slice(&(((DATA as u32) << 8) | count as u32).to_be_bytes());
    let entry_end = (count + 1) * KEY_BYTES;
    after[index + BITMAP + entry_end / 8] |= 1 << (entry_end % 8);
    put16(
        &mut after,
        index + FREE,
        (PAGE - ENTRIES - entry_end) as u16,
    );
    after
}

/// Independently patch an existing nullable Score in historical or current rows.
/// Record counts, absent later fields, indexes and metadata remain byte-exact.
/// Clearing Score preserves its ignored stored payload, changing only its bit.
pub fn update_scores(before: &[u8], updates: &[(usize, Option<i32>)]) -> Vec<u8> {
    const DIRECTORY: usize = 14;
    const ROW_COUNT: usize = 12;
    const SCORE_OFFSET: usize = 6;
    const SCORE_PRESENT: u8 = 2;

    assert_eq!(before.len(), 11 * PAGE);
    let data = DATA * PAGE;
    let count = usize::from(u16::from_le_bytes(
        before[data + ROW_COUNT..data + ROW_COUNT + 2]
            .try_into()
            .unwrap(),
    ));
    let mut after = before.to_vec();
    for (position, &(slot, value)) in updates.iter().enumerate() {
        assert!(slot < count);
        assert!(!updates[..position].iter().any(|(prior, _)| *prior == slot));
        let directory = data + DIRECTORY + slot * 2;
        let root = usize::from(u16::from_le_bytes(
            before[directory..directory + 2].try_into().unwrap(),
        ));
        let start = if root & 0x4000 == 0 {
            assert_eq!(root & !0x1fff, 0);
            root
        } else {
            let pointer = data + (root & 0x1fff);
            let reference = u32::from_le_bytes(before[pointer..pointer + 4].try_into().unwrap());
            assert_eq!(reference >> 8, DATA as u32);
            let target = data + DIRECTORY + (reference as usize & 255) * 2;
            let word = usize::from(u16::from_le_bytes(
                before[target..target + 2].try_into().unwrap(),
            ));
            assert_eq!(word & !0x1fff, 0x8000);
            word & 0x1fff
        };
        let fields = usize::from(u16::from_le_bytes(
            before[data + start..data + start + 2].try_into().unwrap(),
        ));
        assert!([2, 3].contains(&fields));
        let mask = data + start + 2 + fields * 4;
        if let Some(value) = value {
            after[data + start + SCORE_OFFSET..data + start + SCORE_OFFSET + 4]
                .copy_from_slice(&value.to_le_bytes());
            after[mask] |= SCORE_PRESENT;
        } else {
            after[mask] &= !SCORE_PRESENT;
        }
    }
    after
}

#[cfg(all(test, feature = "access-support"))]
mod tests;

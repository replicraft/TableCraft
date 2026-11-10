//! Independent mixed historical rows and variable-footer materialization.
use super::super::storage;
use super::{SourceCase, deleted_field, populated::materialization, source};
use crate::fixtures::access::{
    canonical_initial_table_creation::inserts::initial_rows_image, put16, put32,
};

const PAGE: usize = 4096;
const DEFINITION: usize = 7 * PAGE;
const COLUMN_START: usize = 75;
const COLUMN_BYTES: usize = 25;

#[derive(Clone, Copy, Debug)]
pub enum Case {
    Text,
    Binary,
    LaterAbsentText,
}

fn word(bytes: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()))
}

fn empty(case: Case) -> Vec<u8> {
    let before = source(SourceCase::Mixed);
    if !matches!(case, Case::Binary) {
        return before;
    }
    let mut after = before.to_vec();
    let column = DEFINITION + COLUMN_START + 3 * COLUMN_BYTES;
    after[column] = 9;
    after[column + 11..column + 15].fill(0);
    after[column + 16] = 0;
    put16(&mut after, column + 23, 32);
    let body = deleted_field::without_group(&storage::selected_body(&before), "OtherLabel");
    storage::append(&before, after, &body)
}

/// Explicit raw representations include compressed/uncompressed Text, empty
/// values, NULLs with nonempty opaque payloads, and stationary trailer padding.
pub fn pair(case: Case) -> [Vec<u8>; 2] {
    let mut rows = Vec::new();
    for position in 0..3 {
        let (first, second, score, mask, padding): (&[u8], &[u8], i32, u8, &[u8]) = match position {
            0 => (
                &[255, 254, b'a', b'l', b'p', b'h', b'a'],
                if matches!(case, Case::Binary) {
                    &[0, 255, 1, 2]
                } else {
                    &[3, 38]
                },
                42,
                15,
                &[161, 178],
            ),
            1 => (
                &[255],
                if matches!(case, Case::Binary) {
                    &[]
                } else {
                    &[255, 254]
                },
                0x6a6b6c6d,
                9,
                &[203],
            ),
            _ => (
                &[b'w', 0, b'i', 0, b'd', 0, b'e', 0],
                &[255, 0, 165],
                -7,
                7,
                &[222, 173, 190],
            ),
        };
        let mut row = vec![4, 0];
        row.extend(((position + 1) as i32).to_le_bytes());
        row.extend(score.to_le_bytes());
        let first_end = row.len() + first.len();
        row.extend(first);
        let payload_end = row.len() + second.len();
        row.extend(second);
        row.extend(padding);
        for offset in [payload_end, first_end, 10] {
            row.extend((offset as u16).to_le_bytes());
        }
        row.extend(2_u16.to_le_bytes());
        row.push(mask | 0xa0);
        rows.push((0, row));
    }
    let before = initial_rows_image(&empty(case), rows);
    let mut after = materialization::append_number(&before, "Bonus");
    if matches!(case, Case::LaterAbsentText) {
        let mut zero = after.to_vec();
        for offset in [16, 20, 67] {
            put32(&mut zero, DEFINITION + offset, 0);
        }
        let original_empty = source(SourceCase::Mixed);
        zero[8 * PAGE..10 * PAGE].copy_from_slice(&original_empty[8 * PAGE..10 * PAGE]);
        let mut appended = super::append_text(&zero, "FutureLabel");
        for offset in [16, 20, 67] {
            appended[DEFINITION + offset..DEFINITION + offset + 4]
                .copy_from_slice(&after[DEFINITION + offset..DEFINITION + offset + 4]);
        }
        appended[8 * PAGE..10 * PAGE].copy_from_slice(&after[8 * PAGE..10 * PAGE]);
        after = appended;
    }
    [before, after]
}

/// Independent byte insertion, offset relocation and empty-slot footer growth.
/// Existing payloads are never decoded, compressed or normalized.
pub fn after(before: &[u8], slot: usize, value: i32) -> Vec<u8> {
    let fields = word(before, DEFINITION + 41);
    assert!([5, 6].contains(&fields));
    let start = word(before, 8 * PAGE + 14 + slot * 2);
    assert_eq!(start & !0x1fff, 0);
    let end = if slot == 0 {
        PAGE
    } else {
        word(before, 8 * PAGE + 14 + (slot - 1) * 2) & 0x1fff
    };
    let old = &before[8 * PAGE + start..8 * PAGE + end];
    assert_eq!(word(old, 0), 4);
    let old_mask = old.len() - 1;
    assert_eq!(word(old, old_mask - 2), 2);
    let old_tail = old_mask - 2 - 6;
    let offsets = [
        word(old, old_mask - 4),
        word(old, old_mask - 6),
        word(old, old_mask - 8),
    ];
    assert_eq!(offsets[0], 10);
    assert!(offsets[2] <= old_tail);
    let mut record = old[..10].to_vec();
    put16(&mut record, 0, fields as u16);
    record.extend(value.to_le_bytes());
    record.extend_from_slice(&old[10..old_tail]);
    let variables = if fields == 6 { 3 } else { 2 };
    let mut relocated = offsets.iter().map(|offset| offset + 4).collect::<Vec<_>>();
    if variables == 3 {
        relocated.push(offsets[2] + 4);
    }
    for offset in relocated.into_iter().rev() {
        record.extend((offset as u16).to_le_bytes());
    }
    record.extend((variables as u16).to_le_bytes());
    let mut mask = old[old_mask];
    for number in 4..fields {
        mask &= !(1 << number);
    }
    record.push(mask | 16);
    materialization::place_record(before, slot, &record)
}

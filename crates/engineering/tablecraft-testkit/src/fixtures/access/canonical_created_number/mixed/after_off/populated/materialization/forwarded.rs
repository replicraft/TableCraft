//! Independent second growth: retain the old hidden terminal and retarget root0.
use super::{after, append_number};
use crate::fixtures::access::{put16, put32};

const PAGE: usize = 4096;
const DATA: usize = 8 * PAGE;
const DIRECTORY: usize = 14;

fn word(bytes: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()))
}

/// The first Number is populated in row0, then another declaration is added.
pub fn pair() -> [Vec<u8>; 2] {
    let [_, initial] = super::super::pair("Bonus", &[Some(42), None, Some(-7)]);
    let first = after(&initial, 0, 2, 37);
    let declared = append_number(&first, "SecondBonus");
    [first, declared]
}

pub(in crate::fixtures::access::canonical_created_number::mixed::after_off) fn record(
    before: &[u8],
) -> &[u8] {
    let root = word(before, DATA + DIRECTORY);
    assert_eq!(root & !0x1fff, 0x4000);
    let pointer = DATA + (root & 0x1fff);
    let reference = u32::from_le_bytes(before[pointer..pointer + 4].try_into().unwrap());
    assert_eq!(reference >> 8, 8);
    let physical = reference as usize & 255;
    assert!(physical >= 3 && physical < word(before, DATA + 12));
    let target = word(before, DATA + DIRECTORY + physical * 2);
    assert_eq!(target & !0x1fff, 0x8000);
    let start = target & 0x1fff;
    let end = word(before, DATA + DIRECTORY + (physical - 1) * 2) & 0x1fff;
    &before[DATA + start..DATA + end]
}

/// Materialize only SecondBonus; preserve the previous hidden record verbatim.
pub fn after_second(before: &[u8], value: i32) -> Vec<u8> {
    assert_eq!(word(before, 7 * PAGE + 41), 4);
    let old = record(before);
    assert_eq!(word(old, 0), 3);
    assert_eq!(old.len(), 15);
    let mut row = vec![4, 0];
    row.extend_from_slice(&old[2..14]);
    row.extend(value.to_le_bytes());
    row.push(old[14] | 8);
    place(before, &row)
}

pub(in crate::fixtures::access::canonical_created_number::mixed::after_off) fn place(
    before: &[u8],
    row: &[u8],
) -> Vec<u8> {
    assert!(row.len() > record(before).len());
    let count = word(before, DATA + 12);
    assert!(count < 255);
    let frontier = word(before, DATA + DIRECTORY + (count - 1) * 2) & 0x1fff;
    let target = frontier.checked_sub(row.len()).unwrap();
    let free = word(before, DATA + 2);
    assert_eq!(free, frontier - DIRECTORY - count * 2);
    assert!(target >= DIRECTORY + (count + 1) * 2);
    let root = word(before, DATA + DIRECTORY) & 0x1fff;
    let mut after = before.to_vec();
    after[DATA + target..DATA + frontier].copy_from_slice(row);
    put16(
        &mut after,
        DATA + DIRECTORY + count * 2,
        target as u16 | 0x8000,
    );
    put16(&mut after, DATA + 12, (count + 1) as u16);
    put16(&mut after, DATA + 2, (free - row.len() - 2) as u16);
    put32(&mut after, DATA + root, (8 << 8) | count as u32);
    after
}

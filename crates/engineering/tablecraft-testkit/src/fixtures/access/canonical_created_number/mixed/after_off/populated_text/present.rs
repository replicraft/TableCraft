//! Independent existing-slot Text replacement and logical-root placement.
use crate::fixtures::access::{put16, put32};

const PAGE: usize = 4096;
const DATA: usize = 8 * PAGE;
const DEFINITION: usize = 7 * PAGE;
const DIRECTORY: usize = 14;
const OFFSET_MASK: usize = 0x1fff;
const FORWARDED: usize = 0x4000;
const HIDDEN: usize = 0x8000;

fn word(bytes: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()))
}

fn bounds(bytes: &[u8], slot: usize) -> (usize, usize) {
    let start = word(bytes, DATA + DIRECTORY + slot * 2) & OFFSET_MASK;
    let end = if slot == 0 {
        PAGE
    } else {
        word(bytes, DATA + DIRECTORY + (slot - 1) * 2) & OFFSET_MASK
    };
    (start, end)
}

/// The exact terminal bytes, independently following the single owned pointer.
pub fn record(bytes: &[u8], root: usize) -> &[u8] {
    let (start, end) = bounds(bytes, root);
    if word(bytes, DATA + DIRECTORY + root * 2) & FORWARDED == 0 {
        return &bytes[DATA + start..DATA + end];
    }
    let pointer = u32::from_le_bytes(bytes[DATA + start..DATA + start + 4].try_into().unwrap());
    assert_eq!(pointer >> 8, 8);
    let physical = pointer as usize & 255;
    assert_eq!(
        word(bytes, DATA + DIRECTORY + physical * 2) & !OFFSET_MASK,
        HIDDEN
    );
    let (start, end) = bounds(bytes, physical);
    &bytes[DATA + start..DATA + end]
}

/// Independently replace one physically present field while keeping its stored
/// count and the bytes of every sibling and original padding span unchanged.
pub fn after(before: &[u8], root: usize, column: usize, value: Option<&str>) -> Vec<u8> {
    let old = record(before, root);
    let count = word(old, 0);
    assert!(column < count);
    let field = DEFINITION + 75 + column * 25;
    assert_eq!(before[field], 10);
    let variable = word(before, field + 7);
    let mask = old.len() - count.div_ceil(8);
    if value.is_none() && old[mask + column / 8] & (1 << (column % 8)) == 0 {
        return before.to_vec();
    }
    let variables = word(old, mask - 2);
    assert!(variable < variables);
    let data_end = mask - 2 - 2 * (variables + 1);
    let offsets = (0..=variables)
        .map(|index| word(old, mask - 4 - 2 * index))
        .collect::<Vec<_>>();
    let start = offsets[variable];
    let stop = offsets[variable + 1];
    let end = offsets[variables];
    let mut payload = Vec::new();
    if let Some(value) = value {
        let units = value.encode_utf16().collect::<Vec<_>>();
        assert!(units.len() <= word(before, field + 23) / 2);
        if before[field + 16] & 1 != 0
            && units.len() > 2
            && units.iter().all(|unit| (1..=255).contains(unit))
        {
            payload.extend([255, 254]);
            payload.extend(units.iter().map(|unit| *unit as u8));
        } else {
            assert!(!value.starts_with('\u{feff}'));
            for unit in units {
                payload.extend(unit.to_le_bytes());
            }
        }
    }
    let mut row = old[..start].to_vec();
    row.extend(&payload);
    row.extend_from_slice(&old[stop..end]);
    row.extend_from_slice(&old[end..data_end]);
    let new_data_end = row.len().max(data_end);
    row.resize(new_data_end, 0);
    for (index, offset) in offsets.into_iter().enumerate().rev() {
        let adjusted = if index <= variable {
            offset
        } else {
            offset - (stop - start) + payload.len()
        };
        row.extend((adjusted as u16).to_le_bytes());
    }
    row.extend((variables as u16).to_le_bytes());
    let new_mask = row.len();
    row.extend_from_slice(&old[mask..]);
    row[new_mask + column / 8] &= !(1 << (column % 8));
    if value.is_some() {
        row[new_mask + column / 8] |= 1 << (column % 8);
    }
    place(before, root, &row)
}

/// Same-sized records remain in their original terminal; growth appends one
/// hidden target and retains every previously hidden payload unchanged.
fn place(before: &[u8], root: usize, row: &[u8]) -> Vec<u8> {
    let original = record(before, root);
    let mut after = before.to_vec();
    let root_word = word(before, DATA + DIRECTORY + root * 2);
    let (root_start, root_end) = bounds(before, root);
    if row.len() == original.len() {
        let terminal = if root_word & FORWARDED != 0 {
            let pointer = u32::from_le_bytes(
                before[DATA + root_start..DATA + root_start + 4]
                    .try_into()
                    .unwrap(),
            );
            bounds(before, pointer as usize & 255).0
        } else {
            root_start
        };
        after[DATA + terminal..DATA + terminal + row.len()].copy_from_slice(row);
        return after;
    }
    assert!(row.len() > original.len());
    let count = word(before, DATA + 12);
    assert!(count < 255);
    let frontier = word(before, DATA + DIRECTORY + (count - 1) * 2) & OFFSET_MASK;
    let start = frontier.checked_sub(row.len()).unwrap();
    let free = word(before, DATA + 2);
    assert_eq!(free, frontier - DIRECTORY - 2 * count);
    assert!(start >= DIRECTORY + 2 * (count + 1));
    after[DATA + start..DATA + frontier].copy_from_slice(row);
    put16(
        &mut after,
        DATA + DIRECTORY + count * 2,
        start as u16 | HIDDEN as u16,
    );
    put16(&mut after, DATA + 12, (count + 1) as u16);
    put16(&mut after, DATA + 2, (free - row.len() - 2) as u16);
    if root_word & FORWARDED == 0 {
        after[DATA + root_start..DATA + root_end].fill(0);
    }
    put32(&mut after, DATA + root_start, (8 << 8) | count as u32);
    put16(
        &mut after,
        DATA + DIRECTORY + root * 2,
        root_word as u16 | FORWARDED as u16,
    );
    after
}

//! Independent same-owner terminal selection and logical-root promotion bytes.
use super::{put16, put32};

const PAGE: usize = 4096;
const DIRECTORY: usize = 14;
const OFFSET_MASK: usize = 0x1fff;
const FORWARDED: usize = 0x4000;
const HIDDEN: usize = 0x8000;

fn word(bytes: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()))
}

fn bounds(bytes: &[u8], data_page: u32, slot: usize) -> (usize, usize) {
    let data = data_page as usize * PAGE;
    let start = word(bytes, data + DIRECTORY + slot * 2) & OFFSET_MASK;
    let end = if slot == 0 {
        PAGE
    } else {
        word(bytes, data + DIRECTORY + (slot - 1) * 2) & OFFSET_MASK
    };
    (start, end)
}

/// The exact terminal bytes, independently following the single owned pointer.
pub(in crate::fixtures::access) fn record(bytes: &[u8], data_page: u32, root: usize) -> &[u8] {
    let data = data_page as usize * PAGE;
    assert!(root < word(bytes, data + 12));
    assert_eq!(word(bytes, data + DIRECTORY + root * 2) & HIDDEN, 0);
    let (start, end) = bounds(bytes, data_page, root);
    if word(bytes, data + DIRECTORY + root * 2) & FORWARDED == 0 {
        return &bytes[data + start..data + end];
    }
    let pointer = u32::from_le_bytes(bytes[data + start..data + start + 4].try_into().unwrap());
    assert_eq!(pointer >> 8, data_page);
    let physical = pointer as usize & 255;
    assert_eq!(
        word(bytes, data + DIRECTORY + physical * 2) & !OFFSET_MASK,
        HIDDEN
    );
    let (start, end) = bounds(bytes, data_page, physical);
    &bytes[data + start..data + end]
}

/// Same-sized records remain in their original terminal; growth appends one
/// hidden target and retains every previously hidden payload unchanged.
pub(in crate::fixtures::access) fn place(
    before: &[u8],
    data_page: u32,
    root: usize,
    row: &[u8],
) -> Vec<u8> {
    let data = data_page as usize * PAGE;
    let original = record(before, data_page, root);
    let mut after = before.to_vec();
    let root_word = word(before, data + DIRECTORY + root * 2);
    let (root_start, root_end) = bounds(before, data_page, root);
    if row.len() == original.len() {
        let terminal = if root_word & FORWARDED != 0 {
            let pointer = u32::from_le_bytes(
                before[data + root_start..data + root_start + 4]
                    .try_into()
                    .unwrap(),
            );
            bounds(before, data_page, pointer as usize & 255).0
        } else {
            root_start
        };
        after[data + terminal..data + terminal + row.len()].copy_from_slice(row);
        return after;
    }
    assert!(row.len() > original.len());
    let count = word(before, data + 12);
    assert!(u8::try_from(count).is_ok());
    let frontier = word(before, data + DIRECTORY + (count - 1) * 2) & OFFSET_MASK;
    let start = frontier.checked_sub(row.len()).unwrap();
    let free = word(before, data + 2);
    assert_eq!(free, frontier - DIRECTORY - 2 * count);
    assert!(start >= DIRECTORY + 2 * (count + 1));
    after[data + start..data + frontier].copy_from_slice(row);
    put16(
        &mut after,
        data + DIRECTORY + count * 2,
        start as u16 | HIDDEN as u16,
    );
    put16(&mut after, data + 12, (count + 1) as u16);
    put16(&mut after, data + 2, (free - row.len() - 2) as u16);
    if root_word & FORWARDED == 0 {
        after[data + root_start..data + root_end].fill(0);
    }
    put32(
        &mut after,
        data + root_start,
        (data_page << 8) | count as u32,
    );
    put16(
        &mut after,
        data + DIRECTORY + root * 2,
        root_word as u16 | FORWARDED as u16,
    );
    after
}

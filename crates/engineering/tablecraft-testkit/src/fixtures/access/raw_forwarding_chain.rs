//! Independent same-owner forwarding chains for user and system DATA roots.
use super::{packed_record::append_record, put16, put32};

const PAGE: usize = 4096;
const DIRECTORY: usize = 14;
const OFFSET_MASK: usize = 0x1fff;
const FORWARDED: u16 = 0x4000;
const HIDDEN: u16 = 0x8000;

fn word(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap())
}

fn terminal(before: &[u8], data_page: u32, logical_root: usize) -> (usize, usize, usize) {
    let start = data_page as usize * PAGE;
    let page = &before[start..start + PAGE];
    assert_eq!(&page[..2], &[1, 1]);
    let original_count = usize::from(word(page, 12));
    assert!(logical_root < original_count);
    assert_eq!(word(page, DIRECTORY + 2 * logical_root) & HIDDEN, 0);
    let mut visited = Vec::new();
    let mut terminal = logical_root;
    loop {
        assert!(terminal < original_count);
        assert!(!visited.contains(&terminal));
        visited.push(terminal);
        let entry = word(page, DIRECTORY + 2 * terminal);
        assert_eq!(entry & 0x2000, 0);
        if terminal != logical_root {
            assert_eq!(entry & HIDDEN, HIDDEN);
        }
        if entry & FORWARDED == 0 {
            break;
        }
        let offset = usize::from(entry) & OFFSET_MASK;
        let reference = u32::from_le_bytes(page[offset..offset + 4].try_into().unwrap());
        assert_eq!(reference >> 8, data_page);
        terminal = (reference & 255) as usize;
    }
    let terminal_word = word(page, DIRECTORY + 2 * terminal);
    let terminal_start = usize::from(terminal_word) & OFFSET_MASK;
    let terminal_end = if terminal == 0 {
        PAGE
    } else {
        usize::from(word(page, DIRECTORY + 2 * (terminal - 1))) & OFFSET_MASK
    };
    (terminal, terminal_start, terminal_end)
}

/// Original terminal witness, including through long same-owner chains.
pub(in crate::fixtures::access) fn record(before: &[u8], data_page: u32, root: usize) -> &[u8] {
    let (_, start, end) = terminal(before, data_page, root);
    let page = data_page as usize * PAGE;
    &before[page + start..page + end]
}

/// Extend the actual terminal with additional hidden forwarding slots and an
/// exact terminal copy. Existing logical roots, indexes and table counts remain.
/// Slots/pointers stay inside the chosen owned DATA page; no native writer or
/// production row decoder contributes bytes to this raw construction.
pub fn extend(
    before: &[u8],
    data_page: u32,
    logical_root: usize,
    additional_hops: usize,
) -> Vec<u8> {
    let start = data_page as usize * PAGE;
    let page = &before[start..start + PAGE];
    let original_count = usize::from(word(page, 12));
    let (terminal, terminal_start, terminal_end) = terminal(before, data_page, logical_root);
    if additional_hops == 0 {
        return before.to_vec();
    }
    let count = original_count.checked_add(additional_hops).unwrap();
    assert!(u8::try_from(count - 1).is_ok());
    assert!(data_page <= u32::MAX >> 8);
    let terminal_word = word(page, DIRECTORY + 2 * terminal);
    let original = page[terminal_start..terminal_end].to_vec();
    assert!(original.len() >= 4);
    let owner = page[4..8].to_vec();
    let mut after = before.to_vec();
    let page = &mut after[start..start + PAGE];
    for position in 0..additional_hops {
        let slot = original_count + position;
        let final_record = position + 1 == additional_hops;
        if final_record {
            append_record(page, &original);
        } else {
            let reference = (data_page << 8) | (slot + 1) as u32;
            append_record(page, &reference.to_le_bytes());
        }
        let entry = word(page, DIRECTORY + 2 * slot);
        let flags = if final_record {
            HIDDEN
        } else {
            HIDDEN | FORWARDED
        };
        put16(page, DIRECTORY + 2 * slot, entry | flags);
    }
    put32(
        page,
        terminal_start,
        (data_page << 8) | original_count as u32,
    );
    put16(page, DIRECTORY + 2 * terminal, terminal_word | FORWARDED);
    assert_eq!(&page[4..8], owner);
    assert_eq!(&page[terminal_start + 4..terminal_end], &original[4..]);
    assert_eq!(&after[..start], &before[..start]);
    assert_eq!(&after[start + PAGE..], &before[start + PAGE..]);
    after
}

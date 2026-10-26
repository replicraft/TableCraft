//! Independent leaf framing and row-reference filtering over synthetic goldens.
use super::{ignored_page, numeric_fixture, support};

pub const PAGE_BYTES: usize = 4096;
pub const ROOT: usize = 14;
pub const DATA: usize = 12;
const ENTRY_START: usize = 480;
const MASK_START: usize = 27;
const PREFIX_OFFSET: usize = 24;
const ROW_BYTES: usize = 4;
const ROW_SLOT_MASK: u32 = 255;

pub fn entries(bytes: &[u8], root: usize) -> Vec<Vec<u8>> {
    let page = &bytes[root * PAGE_BYTES..(root + 1) * PAGE_BYTES];
    assert_eq!(&page[..2], &[4, 1]);
    let prefix =
        u16::from_le_bytes(page[PREFIX_OFFSET..PREFIX_OFFSET + 2].try_into().unwrap()) as usize;
    let ends = page[MASK_START..ENTRY_START]
        .iter()
        .enumerate()
        .flat_map(|(byte, value)| {
            (0..8).filter_map(move |bit| (value & (1 << bit) != 0).then_some(byte * 8 + bit))
        });
    let mut previous = 0;
    let mut first = Vec::new();
    let mut result = Vec::new();
    for end in ends {
        let fragment = &page[ENTRY_START + previous..ENTRY_START + end];
        let full = if result.is_empty() {
            first = fragment.to_vec();
            first.clone()
        } else {
            [first[..prefix].to_vec(), fragment.to_vec()].concat()
        };
        result.push(full);
        previous = end;
    }
    result
}

pub fn slot(entry: &[u8]) -> usize {
    let reference = u32::from_be_bytes(entry[entry.len() - ROW_BYTES..].try_into().unwrap());
    assert_eq!(reference >> 8, DATA as u32);
    (reference & ROW_SLOT_MASK) as usize
}

pub fn write_entries(bytes: &mut [u8], root: usize, entries: &[Vec<u8>]) {
    bytes[root * PAGE_BYTES..(root + 1) * PAGE_BYTES]
        .copy_from_slice(&ignored_page::page(true, entries, 0, 0, 0));
}

pub fn ignoring(mut bytes: Vec<u8>, omitted: &[usize], unique: bool) -> Vec<u8> {
    let retained: Vec<_> = entries(&bytes, ROOT)
        .into_iter()
        .filter(|entry| !omitted.contains(&slot(entry)))
        .collect();
    numeric_fixture::index_flags(&mut bytes, if unique { 0x83 } else { 0x82 });
    write_entries(&mut bytes, ROOT, &retained);
    bytes
}

pub fn integer_pair(code: u8, descending: bool, unique: bool) -> Vec<u8> {
    let rows = [
        vec![Some(1), None, None],
        vec![Some(2), None, Some(0)],
        vec![Some(3), Some(0), None],
        vec![Some(4), Some(0), Some(0)],
        vec![Some(5), None, None],
        vec![Some(6), Some(1), None],
    ];
    let mut bytes = numeric_fixture::database(
        code,
        &[4, 4, 4],
        &[(2, descending), (1, !descending)],
        &rows,
        0,
    );
    if code == 1 {
        bytes[4..20].copy_from_slice(b"Standard Jet DB\0");
    }
    bytes = ignoring(bytes, &[0, 4], unique);
    bytes
}

pub fn wrong_second_component(bytes: &mut [u8], slot: usize) {
    const SLOT_START: usize = 14;
    const SLOT_BYTES: usize = 2;
    const ROW_OFFSET_MASK: u16 = 0x1fff;
    const SECOND_COMPONENT_OFFSET: usize = 10;
    let page = &mut bytes[DATA * PAGE_BYTES..(DATA + 1) * PAGE_BYTES];
    let at = SLOT_START + slot * SLOT_BYTES;
    let start = (u16::from_le_bytes(page[at..at + SLOT_BYTES].try_into().unwrap())
        & ROW_OFFSET_MASK) as usize;
    support::put32(page, start + SECOND_COMPONENT_OFFSET, 99);
}

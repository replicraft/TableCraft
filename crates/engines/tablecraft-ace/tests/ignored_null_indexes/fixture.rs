//! Original literal Int32 index entries; no production encoding is used.
use super::{index_fixture, support};

pub const PAGE_BYTES: usize = 4096;
pub const DEFINITION_PAGE: usize = 10;
pub const INDEX_FLAGS: usize = DEFINITION_PAGE * PAGE_BYTES + 189;
pub const ROOT: usize = 13;
pub const LEFT: usize = 15;
pub const RIGHT: usize = 16;
pub const ENTRY_START: usize = 480;
pub const ROW_PAGE: usize = 12;
const INDEX_USAGE: usize = 14;

pub fn key(value: Option<i32>, slot: usize, descending: bool) -> Vec<u8> {
    let mut bytes = vec![match (value, descending) {
        (None, false) => 0,
        (None, true) => 255,
        (Some(_), false) => 127,
        (Some(_), true) => 128,
    }];
    if let Some(value) = value {
        bytes.extend(
            ((value as u32) ^ 0x8000_0000)
                .to_be_bytes()
                .map(|byte| if descending { !byte } else { byte }),
        );
    }
    bytes.extend((((ROW_PAGE as u32) << 8) | slot as u32).to_be_bytes());
    bytes
}

pub fn page(leaf: bool, entries: &[Vec<u8>], previous: u32, next: u32, tail: u32) -> Vec<u8> {
    let mut page = vec![0; PAGE_BYTES];
    page[..2].copy_from_slice(&[if leaf { 4 } else { 3 }, 1]);
    page[26] = u8::from(!leaf);
    support::put32(&mut page, 4, DEFINITION_PAGE as u32);
    support::put32(&mut page, 12, previous);
    support::put32(&mut page, 16, next);
    support::put32(&mut page, 20, tail);
    let mut end = 0;
    for entry in entries {
        page[ENTRY_START + end..ENTRY_START + end + entry.len()].copy_from_slice(entry);
        end += entry.len();
        page[27 + end / 8] |= 1 << (end % 8);
    }
    support::put16(&mut page, 2, (PAGE_BYTES - ENTRY_START - end) as u16);
    page
}

pub fn database(
    code: u8,
    descending: bool,
    unique: bool,
    node: bool,
    values: &[Option<i32>],
) -> Vec<u8> {
    let mut bytes = index_fixture::database(descending, true, true);
    bytes[20] = code;
    if code != 1 {
        bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    }
    bytes[INDEX_FLAGS] = if unique { 0x83 } else { 0x82 };
    support::put32(
        &mut bytes,
        DEFINITION_PAGE * PAGE_BYTES + 16,
        values.len() as u32,
    );
    let rows: Vec<_> = values
        .iter()
        .enumerate()
        .map(|(position, value)| {
            let mut row = 2_u16.to_le_bytes().to_vec();
            row.extend((position as i32 + 1).to_le_bytes());
            row.extend(value.unwrap_or(0).to_le_bytes());
            row.push(if value.is_some() { 3 } else { 1 });
            (0, row)
        })
        .collect();
    bytes[ROW_PAGE * PAGE_BYTES..(ROW_PAGE + 1) * PAGE_BYTES]
        .copy_from_slice(&support::data_page(DEFINITION_PAGE as u32, &rows));
    let mut entries: Vec<_> = values
        .iter()
        .copied()
        .enumerate()
        .filter_map(|(slot, value)| value.map(|value| (value, slot)))
        .collect();
    entries.sort_by(|(left, a), (right, b)| {
        let order = left.cmp(right);
        (if descending { order.reverse() } else { order }).then(a.cmp(b))
    });
    let entries: Vec<_> = entries
        .into_iter()
        .map(|(value, slot)| key(Some(value), slot, descending))
        .collect();
    if node {
        assert!(entries.len() >= 2);
        bytes.resize((RIGHT + 1) * PAGE_BYTES, 0);
        let split = entries.len() / 2;
        let mut separator = entries[split - 1].clone();
        separator.extend((LEFT as u32).to_be_bytes());
        bytes[ROOT * PAGE_BYTES..(ROOT + 1) * PAGE_BYTES].copy_from_slice(&page(
            false,
            &[separator],
            0,
            0,
            RIGHT as u32,
        ));
        bytes[LEFT * PAGE_BYTES..(LEFT + 1) * PAGE_BYTES].copy_from_slice(&page(
            true,
            &entries[..split],
            0,
            RIGHT as u32,
            0,
        ));
        bytes[RIGHT * PAGE_BYTES..(RIGHT + 1) * PAGE_BYTES].copy_from_slice(&page(
            true,
            &entries[split..],
            LEFT as u32,
            0,
            0,
        ));
        bytes[INDEX_USAGE * PAGE_BYTES..(INDEX_USAGE + 1) * PAGE_BYTES].copy_from_slice(
            &support::data_page(1, &[(0, vec![0, 0, 0, 0, 0, 0, 0xa0, 1])]),
        );
    } else {
        bytes[ROOT * PAGE_BYTES..(ROOT + 1) * PAGE_BYTES]
            .copy_from_slice(&page(true, &entries, 0, 0, 0));
    }
    bytes
}

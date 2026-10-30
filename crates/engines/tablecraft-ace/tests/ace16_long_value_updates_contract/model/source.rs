//! Actual shared source constructor, extracted without changing literal bytes.
use super::{fixture, support};

pub const PAGE: usize = 4096;
pub const ROOT: usize = 12;
pub const MEMO: usize = 13;
pub const OLE: usize = 14;
pub const EOF: usize = 24;
pub const SLOT: usize = 14;
pub const MAP_HEADER: usize = 5;
pub const DECLARATIONS: usize = 162;

pub fn page(owner: u32, rows: &[(u16, Vec<u8>)]) -> Vec<u8> {
    let mut bytes = support::data_page(owner, rows);
    let body: usize = rows.iter().map(|(_, row)| row.len()).sum();
    support::put16(&mut bytes, 2, (PAGE - SLOT - 2 * rows.len() - body) as u16);
    bytes
}

pub fn lval(rows: &[(u16, Vec<u8>)]) -> Vec<u8> {
    let mut bytes = page(0, rows);
    bytes[4..8].copy_from_slice(b"LVAL");
    bytes
}

pub fn map(numbers: &[usize]) -> Vec<u8> {
    let mut bytes = vec![0; 69];
    for &number in numbers {
        bytes[MAP_HEADER + number / 8] |= 1 << (number % 8);
    }
    bytes
}

pub fn descriptor(tag: u32, payload_bytes: usize, reference: u32) -> Vec<u8> {
    let mut bytes = (tag | payload_bytes as u32).to_le_bytes().to_vec();
    bytes.extend(reference.to_le_bytes());
    bytes.extend([0; 4]);
    bytes
}

pub fn single(length: usize, page: usize, slot: usize) -> Vec<u8> {
    descriptor(0x4000_0000, length, ((page << 8) | slot) as u32)
}

pub fn record(id: i32, memo: Option<&[u8]>, ole: Option<&[u8]>) -> Vec<u8> {
    let mut bytes = vec![3, 0];
    bytes.extend(id.to_le_bytes());
    if let Some(value) = memo {
        bytes.extend(value);
    }
    let middle = bytes.len() as u16;
    if let Some(value) = ole {
        bytes.extend(value);
    }
    let end = bytes.len() as u16;
    bytes.extend(end.to_le_bytes());
    bytes.extend(middle.to_le_bytes());
    bytes.extend(6_u16.to_le_bytes());
    bytes.extend(2_u16.to_le_bytes());
    bytes.push(1 | if memo.is_some() { 2 } else { 0 } | if ole.is_some() { 4 } else { 0 });
    bytes
}

pub fn install(bytes: &mut [u8], number: usize, contents: &[u8]) {
    bytes[number * PAGE..(number + 1) * PAGE].copy_from_slice(contents);
}

pub fn set_maps(bytes: &mut [u8], memo_owned: &[usize], memo_hints: &[usize], ole_owned: &[usize]) {
    let rows = vec![
        (0, map(&[ROOT])),
        (0, map(&[ROOT])),
        (0, map(memo_owned)),
        (0, map(memo_hints)),
        (0, map(ole_owned)),
        (0, map(ole_owned)),
    ];
    install(bytes, 11, &page(0, &rows));
}

pub fn source(code: u8, roots: usize) -> Vec<u8> {
    let mut bytes = fixture::database();
    bytes.resize(EOF * PAGE, 0);
    // This fresh literal fixture has no system-column orphan ownership. Give
    // both complete declarations, rather than asking a writer to infer that
    // zero-filled unknown suffixes mean an empty historical inventory.
    let catalog_fields = [
        "Id", "ParentId", "Name", "Type", "Flags", "LvProp", "Lv", "LvExtra", "LvModule",
    ];
    let relationship_fields = [
        "szRelationship",
        "grbit",
        "ccolumn",
        "icolumn",
        "szReferencedObject",
        "szObject",
        "szReferencedColumn",
        "szColumn",
    ];
    for (number, names) in [
        (2, catalog_fields.as_slice()),
        (7, relationship_fields.as_slice()),
    ] {
        let names_bytes: usize = names
            .iter()
            .map(|name| 2 + name.encode_utf16().count() * 2)
            .sum();
        let tail = 63 + names.len() * 25 + names_bytes;
        assert_eq!(tail, if number == 2 { 398 } else { 449 });
        support::put16(&mut bytes, number * PAGE + tail, u16::MAX);
    }
    bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    bytes[20] = code;
    let mut definition = fixture::definition(
        &[("Id", 4, 0, 4), ("Memo", 11, 0, 0), ("Ole", 11, 0, 0)],
        roots as u32,
        11 << 8,
    );
    definition[88] = 12;
    definition[103] = 2;
    definition[128] = 2;
    support::put32(&mut definition, 59, (11 << 8) | 1);
    for (at, column, owned, hint) in [
        (DECLARATIONS, 1, (11 << 8) | 2, (11 << 8) | 3),
        (DECLARATIONS + 10, 2, (11 << 8) | 4, (11 << 8) | 5),
    ] {
        support::put16(&mut definition, at, column);
        support::put32(&mut definition, at + 2, owned);
        support::put32(&mut definition, at + 6, hint);
    }
    support::put16(&mut definition, DECLARATIONS + 20, u16::MAX);
    install(&mut bytes, 10, &definition);
    set_maps(&mut bytes, &[MEMO], &[MEMO], &[OLE]);
    let memo = single(2, MEMO, 0);
    let ole = single(1, OLE, 0);
    let rows = (0..roots)
        .map(|ordinal| (0, record(ordinal as i32 + 1, Some(&memo), Some(&ole))))
        .collect::<Vec<_>>();
    install(&mut bytes, ROOT, &page(10, &rows));
    install(
        &mut bytes,
        MEMO,
        &lval(&[(0, vec![65, 0]), (0, vec![0xf1, 0xe2, 0xd3])]),
    );
    install(&mut bytes, OLE, &lval(&[(0, vec![7])]));
    let free: Vec<_> = (EOF..512).collect();
    install(&mut bytes, 1, &page(1, &[(0, map(&free))]));
    bytes
}

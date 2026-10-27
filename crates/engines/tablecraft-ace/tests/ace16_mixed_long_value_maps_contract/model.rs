use super::*;

pub const PAGE: usize = 4096;
pub const ROOT: usize = 12;
pub const MEMO: usize = 13;
pub const OLE: usize = 14;
pub const BITMAP: usize = 19;
pub const EOF: usize = 24;
pub const SLOT: usize = 14;
pub const OFFSET_MASK: u16 = 0x1fff;
pub const MAP_HEADER: usize = 5;
pub const BITMAP_HEADER: usize = 4;
pub const DECLARATIONS: usize = 162;
#[cfg(feature = "ace16-write-preview")]
const DESCRIPTOR_OFFSET: usize = 6;
#[cfg(feature = "ace16-write-preview")]
const DESCRIPTOR_BYTES: usize = 12;
const TABLE_HEADER: usize = 63;
const COLUMN_BYTES: usize = 25;
const TERMINATOR: u16 = 0xffff;

pub fn budget() -> ResourceBudget {
    ResourceBudget::new(128 * 1024 * 1024, 100_000, 50_000_000)
}

pub fn page(owner: u32, rows: &[(u16, Vec<u8>)]) -> Vec<u8> {
    let mut result = support::data_page(owner, rows);
    let used = rows.iter().map(|(_, row)| row.len() + 2).sum::<usize>();
    support::put16(&mut result, 2, (PAGE - SLOT - used) as u16);
    result
}

pub fn lval(rows: &[(u16, Vec<u8>)]) -> Vec<u8> {
    let mut result = page(0, rows);
    result[4..8].copy_from_slice(b"LVAL");
    result
}

pub fn inline_map(members: &[usize]) -> Vec<u8> {
    let mut result = vec![0; 69];
    for &number in members {
        result[MAP_HEADER + number / 8] |= 1 << (number % 8);
    }
    result
}

pub fn indirect(pointer: usize) -> Vec<u8> {
    let mut result = vec![1];
    result.extend((pointer as u32).to_le_bytes());
    result
}

pub fn bitmap(members: &[usize]) -> Vec<u8> {
    let mut result = vec![0; PAGE];
    // Independent literal four-byte native bitmap signature. Candidate edits
    // affect represented bits and preserve every byte of this header.
    result[..BITMAP_HEADER].copy_from_slice(&[5, 1, 0, 0]);
    for &number in members {
        result[BITMAP_HEADER + number / 8] |= 1 << (number % 8);
    }
    result
}

pub fn descriptor(tag: u32, length: usize, reference: u32) -> Vec<u8> {
    let mut result = (tag | length as u32).to_le_bytes().to_vec();
    result.extend(reference.to_le_bytes());
    result.extend([0; 4]);
    result
}

pub fn single(length: usize, number: usize, slot: usize) -> Vec<u8> {
    descriptor(0x4000_0000, length, ((number << 8) | slot) as u32)
}

pub fn inline(payload: &[u8]) -> Vec<u8> {
    let mut result = descriptor(0x8000_0000, payload.len(), 0);
    result.extend(payload);
    result
}

pub fn record(id: i32, memo: &[u8], ole: &[u8]) -> Vec<u8> {
    let mut result = vec![3, 0];
    result.extend(id.to_le_bytes());
    result.extend(memo);
    let middle = result.len() as u16;
    result.extend(ole);
    let end = result.len() as u16;
    result.extend(end.to_le_bytes());
    result.extend(middle.to_le_bytes());
    result.extend(6_u16.to_le_bytes());
    result.extend(2_u16.to_le_bytes());
    result.push(7);
    result
}

pub fn install(bytes: &mut [u8], number: usize, contents: &[u8]) {
    bytes[number * PAGE..(number + 1) * PAGE].copy_from_slice(contents);
}

pub fn source(code: u8, external: bool, pointer: usize) -> Vec<u8> {
    let mut result = fixture::database();
    result.resize(EOF * PAGE, 0);
    result[4..20].copy_from_slice(b"Standard ACE DB\0");
    result[20] = code;
    let catalog = [
        "Id", "ParentId", "Name", "Type", "Flags", "LvProp", "Lv", "LvExtra", "LvModule",
    ];
    let relationships = [
        "szRelationship",
        "grbit",
        "ccolumn",
        "icolumn",
        "szReferencedObject",
        "szObject",
        "szReferencedColumn",
        "szColumn",
    ];
    for (definition, names) in [(2, catalog.as_slice()), (7, relationships.as_slice())] {
        let name_bytes = names
            .iter()
            .map(|name| 2 + name.encode_utf16().count() * 2)
            .sum::<usize>();
        let tail = TABLE_HEADER + names.len() * COLUMN_BYTES + name_bytes;
        support::put16(&mut result, definition * PAGE + tail, TERMINATOR);
    }
    let mut definition = fixture::definition(
        &[("Id", 4, 0, 4), ("Memo", 11, 0, 0), ("Ole", 11, 0, 0)],
        1,
        11 << 8,
    );
    // Native Memo is variable, ordinary writable, with no extended flags.
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
    support::put16(&mut definition, DECLARATIONS + 20, TERMINATOR);
    install(&mut result, 10, &definition);
    let owned = if external { vec![MEMO] } else { Vec::new() };
    let rows = vec![
        (0, inline_map(&[ROOT])),
        (0, inline_map(&[ROOT])),
        (0, indirect(pointer)),
        (0, inline_map(&owned)),
        (0, inline_map(&[OLE])),
        (0, inline_map(&[OLE])),
    ];
    install(&mut result, 11, &page(0, &rows));
    install(&mut result, BITMAP, &bitmap(&owned));
    let memo = if external {
        single(2, MEMO, 0)
    } else {
        inline(&[65, 0])
    };
    let ole = single(1, OLE, 0);
    install(&mut result, ROOT, &page(10, &[(0, record(1, &memo, &ole))]));
    install(
        &mut result,
        MEMO,
        &lval(&[(0, vec![65, 0]), (0, vec![0xf1, 0xe2, 0xd3])]),
    );
    install(&mut result, OLE, &lval(&[(0, vec![7])]));
    install(
        &mut result,
        1,
        &page(1, &[(0, inline_map(&(EOF..512).collect::<Vec<_>>()))]),
    );
    result
}

pub fn start(bytes: &[u8], number: usize, slot: usize) -> usize {
    let word = u16::from_le_bytes(
        bytes[number * PAGE + SLOT + slot * 2..number * PAGE + SLOT + slot * 2 + 2]
            .try_into()
            .unwrap(),
    );
    number * PAGE + usize::from(word & OFFSET_MASK)
}

pub fn set_inline(bytes: &mut [u8], container: usize, row: usize, number: usize, set: bool) {
    let offset = start(bytes, container, row) + MAP_HEADER + number / 8;
    let mask = 1 << (number % 8);
    if set {
        bytes[offset] |= mask;
    } else {
        bytes[offset] &= !mask;
    }
}

pub fn set_owned(bytes: &mut [u8], number: usize, set: bool) {
    let offset = BITMAP * PAGE + BITMAP_HEADER + number / 8;
    let mask = 1 << (number % 8);
    if set {
        bytes[offset] |= mask;
    } else {
        bytes[offset] &= !mask;
    }
}

#[cfg(feature = "ace16-write-preview")]
pub fn select_descriptor(bytes: &mut [u8], value: &[u8]) {
    assert_eq!(value.len(), DESCRIPTOR_BYTES);
    let offset = start(bytes, ROOT, 0) + DESCRIPTOR_OFFSET;
    bytes[offset..offset + DESCRIPTOR_BYTES].copy_from_slice(value);
}

pub fn checked(bytes: &[u8]) -> tablecraft_ace::TableLongValueCheck {
    check_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    check_table_constraints(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    check_table_long_values(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

pub fn read(bytes: &[u8]) -> Vec<Vec<Value>> {
    read_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap()
    .rows()
    .to_vec()
}

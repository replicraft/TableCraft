//! Independent native17 SOURCE projection of immutable authored canonical originals.
//! No creation writer, metadata decoder, normal planner or expected output is used.
use super::{data_page, put16, put32};

const PAGE: usize = 4096;

// name, native type, fixed offset (None means variable), width, native variable,
// system descriptor flags. These literal declarations are independent goldens.
type CatalogFieldSpec = (&'static str, u8, Option<u16>, u16, u16, u8);

const FIELDS: [CatalogFieldSpec; 17] = [
    ("Id", 4, Some(0), 4, 0, 0x13),
    ("ParentId", 4, Some(4), 4, 0, 0x13),
    ("Name", 10, None, 510, 0, 0x12),
    ("Type", 3, Some(8), 2, 1, 0x13),
    ("DateCreate", 8, Some(10), 8, 1, 0x13),
    ("DateUpdate", 8, Some(18), 8, 1, 0x13),
    ("Owner", 9, None, 510, 1, 0x32),
    ("Flags", 4, Some(26), 4, 2, 0x13),
    ("Database", 12, None, 0, 2, 0x12),
    ("Connect", 12, None, 0, 3, 0x12),
    ("ForeignName", 10, None, 510, 4, 0x12),
    ("RmtInfoShort", 9, None, 510, 5, 0x12),
    ("RmtInfoLong", 11, None, 0, 6, 0x12),
    ("Lv", 11, None, 0, 7, 0x12),
    ("LvProp", 11, None, 0, 8, 0x12),
    ("LvModule", 11, None, 0, 9, 0x12),
    ("LvExtra", 11, None, 0, 10, 0x12),
];

fn word(raw: &[u8], at: usize) -> u16 {
    u16::from_le_bytes(raw[at..at + 2].try_into().unwrap())
}

fn long(raw: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(raw[at..at + 4].try_into().unwrap())
}

fn records(page: &[u8]) -> Vec<Vec<u8>> {
    assert_eq!(&page[..2], &[1, 1]);
    let count = usize::from(word(page, 12));
    let mut rows = Vec::new();
    let mut end = PAGE;
    for position in 0..count {
        let start = usize::from(word(page, 14 + 2 * position));
        assert!(start <= end && start >= 14 + 2 * count);
        rows.push(page[start..end].to_vec());
        end = start;
    }
    assert_eq!(usize::from(word(page, 2)), end - 14 - 2 * count);
    rows
}

fn packed(owner: u32, rows: Vec<Vec<u8>>) -> Vec<u8> {
    let count = rows.len();
    let payload = rows.iter().map(Vec::len).sum::<usize>();
    assert!(14 + 2 * count + payload <= PAGE);
    let rows = rows.into_iter().map(|row| (0, row)).collect::<Vec<_>>();
    let mut page = data_page(owner, &rows);
    put16(&mut page, 2, (PAGE - 14 - 2 * count - payload) as u16);
    page
}

fn declaration(rows: usize) -> Vec<u8> {
    let mut page = vec![0; PAGE];
    page[..2].copy_from_slice(&[2, 1]);
    put32(&mut page, 12, 1625);
    put32(&mut page, 16, rows as u32);
    page[24] = 1;
    page[40] = 0x53;
    put16(&mut page, 41, 17);
    put16(&mut page, 43, 11);
    put16(&mut page, 45, 17);
    put32(&mut page, 55, 3 << 8);
    put32(&mut page, 59, (3 << 8) | 4);
    let mut names = 63 + 25 * 17;
    for (number, (name, kind, fixed, width, variable, flags)) in FIELDS.iter().enumerate() {
        let at = 63 + 25 * number;
        page[at] = *kind;
        put32(&mut page, at + 1, 1625);
        put16(&mut page, at + 5, number as u16);
        put16(&mut page, at + 7, *variable);
        put16(&mut page, at + 9, number as u16);
        if *kind == 10 {
            page[at + 11..at + 15].copy_from_slice(&[9, 4, 0, 1]);
        }
        page[at + 15] = *flags;
        put16(&mut page, at + 21, fixed.unwrap_or(0));
        put16(&mut page, at + 23, *width);
        let encoded = name
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        put16(&mut page, names, encoded.len() as u16);
        names += 2;
        page[names..names + encoded.len()].copy_from_slice(&encoded);
        names += encoded.len();
    }
    for (position, field) in [8_u16, 9, 12, 13, 14, 15, 16].into_iter().enumerate() {
        put16(&mut page, names, field);
        put32(&mut page, names + 2, (3 << 8) | (8 + 2 * position as u32));
        put32(&mut page, names + 6, (3 << 8) | (9 + 2 * position as u32));
        names += 10;
    }
    put16(&mut page, names, 0xffff);
    let length = names + 2;
    put32(&mut page, 8, length as u32);
    put16(&mut page, 2, (PAGE - length - 8) as u16);
    page
}

fn catalog_row(original: &[u8]) -> Vec<u8> {
    assert_eq!(word(original, 0), 15);
    assert_eq!(&original[original.len() - 2..], &[0x1f, 0x60]);
    assert_eq!(word(original, original.len() - 4), 9);
    // Name is the old source's sole present variable payload. Its qualified
    // raw escape bytes are retained, not decoded and re-encoded by a writer.
    let name = &original[32..original.len() - 24];
    let eod = 32 + name.len();
    let mut raw = vec![0; 61 + name.len()];
    put16(&mut raw, 0, 17);
    raw[2..12].copy_from_slice(&original[2..12]);
    raw[12..20].copy_from_slice(&original[16..24]);
    raw[20..28].copy_from_slice(&original[24..32]);
    raw[28..32].copy_from_slice(&original[12..16]);
    raw[32..eod].copy_from_slice(name);
    let mut at = eod;
    put16(&mut raw, at, eod as u16);
    at += 2;
    for slot in (0..11).rev() {
        put16(&mut raw, at, if slot == 0 { 32 } else { eod } as u16);
        at += 2;
    }
    put16(&mut raw, at, 11);
    raw[at + 2..].copy_from_slice(&[0xbf, 0, 0]);
    raw
}

/// Only the independent canonical15 source constructors may supply this input.
/// All original bytes outside catalog TD/DATA, map container and appended user
/// map references are retained exactly. This is not imported-file admission.
pub fn source(mut original: Vec<u8>) -> Vec<u8> {
    assert!(original.len() >= 9 * PAGE && original.len().is_multiple_of(PAGE));
    assert_eq!(word(&original[2 * PAGE..], 41), 15);
    let catalog = records(&original[4 * PAGE..5 * PAGE]);
    let old_maps = records(&original[3 * PAGE..4 * PAGE]);
    assert!(old_maps.len() >= 16);
    for map in &old_maps {
        assert_eq!(map.len(), 13);
        assert_eq!(&map[..5], &[0; 5]);
    }
    assert!(old_maps[8..16].iter().all(|map| map.as_slice() == [0; 13]));
    let mut maps = old_maps[..8].to_vec();
    maps.extend((0..14).map(|_| vec![0; 13]));
    maps.extend(old_maps[16..].iter().cloned());
    let definition = declaration(catalog.len());
    let data = packed(2, catalog.iter().map(|row| catalog_row(row)).collect());
    let map_page = packed(0, maps);
    original[2 * PAGE..3 * PAGE].copy_from_slice(&definition);
    original[3 * PAGE..4 * PAGE].copy_from_slice(&map_page);
    original[4 * PAGE..5 * PAGE].copy_from_slice(&data);
    for row in catalog {
        let id = long(&row, 2) as usize;
        let flags = long(&row, 12);
        if id > 7 && id < original.len() / PAGE && flags == 0 {
            for offset in [55, 59] {
                let at = id * PAGE + offset;
                let old = long(&original, at);
                assert_eq!(old >> 8, 3);
                assert!(old & 255 >= 16);
                put32(&mut original, at, old + 6);
            }
        }
    }
    original
}

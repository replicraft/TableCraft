//! Literal SDK-qualified ADD DefaultValue-zero bytes, independent of ACE codecs.
use super::{canonical_created_add_column, canonical_initial_table_creation, put16, put32};

const PAGE: usize = 4096;
const ROOT: usize = 7;
const PROPERTY_VARIABLE: usize = 8;
const MASK_BYTES: usize = 3;

#[derive(Clone, Copy, Debug)]
pub enum Source {
    /// Ordinary first GUI table: Table1(ID Increment primary PrimaryKey).
    IdOnly,
    /// Items(Id Increment primary PrimaryId, Score LONG DefaultValue0).
    ExistingZero,
}

fn word(bytes: &[u8], at: usize) -> usize {
    u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()) as usize
}

fn wide(text: &str) -> Vec<u8> {
    text.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

/// Reauthor the one-field raw creator fixture with the actual first-table names.
/// No production creator, parser, writer or SDK participates in this oracle.
pub fn source(case: Source) -> Vec<u8> {
    match case {
        Source::ExistingZero => canonical_created_add_column::default_zero_source(),
        Source::IdOnly => {
            let mut bytes = canonical_initial_table_creation::source(
                canonical_initial_table_creation::Case::IdOnly,
            )
            .bytes;
            let page = &mut bytes[7 * PAGE..8 * PAGE];
            // One 25-byte declaration follows the 75-byte native indexed header.
            assert_eq!(&page[100..106], &[4, 0, b'I', 0, b'd', 0]);
            page[104] = b'D';
            let old = wide("PrimaryId");
            let position = page
                .windows(old.len())
                .position(|value| value == old)
                .unwrap();
            let length = u32::from_le_bytes(page[8..12].try_into().unwrap()) as usize;
            let replacement = wide("PrimaryKey");
            let mut definition = page[..position - 2].to_vec();
            definition.extend((replacement.len() as u16).to_le_bytes());
            definition.extend(replacement);
            definition.extend(&page[position + old.len()..length]);
            let length = definition.len();
            put16(&mut definition, 2, (PAGE - length - 8) as u16);
            put32(&mut definition, 8, length as u32);
            definition.resize(PAGE, 0);
            page.copy_from_slice(&definition);

            let catalog = &mut bytes[4 * PAGE..5 * PAGE];
            let start = word(catalog, 14 + 2 * ROOT);
            let end = word(catalog, 14 + 2 * (ROOT - 1));
            let old = catalog[start..end].to_vec();
            assert_eq!(&old[32..42], wide("Items"));
            let mut row = old[..32].to_vec();
            row.extend(wide("Table1"));
            row.extend(&old[42..]);
            let count_position = row.len() - MASK_BYTES - 2;
            assert_eq!(word(&row, count_position), 11);
            for index in 1..=11 {
                let position = count_position - 2 * (index + 1);
                put16(&mut row, position, 44);
            }
            let new_start = end - row.len();
            catalog[new_start..end].copy_from_slice(&row);
            put16(catalog, 14 + 2 * ROOT, new_start as u16);
            let free = word(catalog, 2);
            put16(catalog, 2, (free - 2) as u16);
            bytes
        }
    }
}

fn group(name: &str) -> Vec<u8> {
    let name = wide(name);
    let mut bytes = ((22 + name.len()) as u32).to_le_bytes().to_vec();
    bytes.extend(1_u16.to_le_bytes());
    bytes.extend(((6 + name.len()) as u32).to_le_bytes());
    bytes.extend((name.len() as u16).to_le_bytes());
    bytes.extend(name);
    // Native property: DDL Memo, dictionary index0, two-byte UTF16 literal0.
    bytes.extend([10, 0, 1, 12, 0, 0, 2, 0, b'0', 0]);
    bytes
}

fn property_body(row: &[u8], name: &str) -> Vec<u8> {
    let count_position = row.len() - MASK_BYTES - 2;
    let start = word(row, count_position - 2 * (PROPERTY_VARIABLE + 1));
    let end = word(row, count_position - 2 * (PROPERTY_VARIABLE + 2));
    let mut bytes = if start == end {
        let name = wide("DefaultValue");
        let mut bytes = b"MR2\0".to_vec();
        bytes.extend(((8 + name.len()) as u32).to_le_bytes());
        bytes.extend([128, 0]);
        bytes.extend((name.len() as u16).to_le_bytes());
        bytes.extend(name);
        bytes
    } else {
        let length = u32::from_le_bytes(row[start..start + 4].try_into().unwrap());
        assert_eq!(length & 0xc000_0000, 0x8000_0000);
        assert_eq!(end - start, 12 + (length & 0x3fff_ffff) as usize);
        row[start + 12..end].to_vec()
    };
    bytes.extend(group(name));
    bytes
}

/// Complete AFTER, including literal hidden-row forwarding and one LVAL page.
/// Only the raw source helpers and literal native geometry are used.
pub fn append(before: &[u8], name: &str) -> Vec<u8> {
    assert_eq!(before.len(), 10 * PAGE);
    let mut after = canonical_created_add_column::append(before, name);
    let page = &before[4 * PAGE..5 * PAGE];
    assert_eq!(word(page, 12), 8);
    let start = word(page, 14 + 2 * ROOT);
    let end = word(page, 14 + 2 * (ROOT - 1));
    let old = &page[start..end];
    let count_position = old.len() - MASK_BYTES - 2;
    let begin = word(old, count_position - 2 * (PROPERTY_VARIABLE + 1));
    let finish = word(old, count_position - 2 * (PROPERTY_VARIABLE + 2));
    let body = property_body(old, name);
    let length = old.len().max(old.len() - (finish - begin) + 12);
    let mut row = vec![0; length];
    row[..begin].copy_from_slice(&old[..begin]);
    put32(&mut row, begin, 0x4000_0000 | body.len() as u32);
    put32(&mut row, begin + 4, 10 << 8);
    let footer_bytes = old.len() - finish;
    row[length - footer_bytes..].copy_from_slice(&old[finish..]);
    let count_position = row.len() - MASK_BYTES - 2;
    for index in PROPERTY_VARIABLE + 1..=11 {
        put16(
            &mut row,
            count_position - 2 * (index + 1),
            (begin + 12) as u16,
        );
    }
    let mask = row.len() - MASK_BYTES;
    row[mask + 1] |= 0x40;
    let catalog = &mut after[4 * PAGE..5 * PAGE];
    if length == old.len() {
        catalog[start..end].copy_from_slice(&row);
    } else {
        let target = start - length;
        catalog[target..start].copy_from_slice(&row);
        catalog[start..end].fill(0);
        put32(catalog, start, (4 << 8) | 8);
        put16(catalog, 14 + 2 * ROOT, start as u16 | 0x4000);
        put16(catalog, 14 + 2 * 8, target as u16 | 0x8000);
        put16(catalog, 12, 9);
        put16(catalog, 2, (target - (14 + 2 * 9)) as u16);
    }
    for slot in [16, 17] {
        let maps = &mut after[3 * PAGE..4 * PAGE];
        let start = word(maps, 14 + 2 * slot);
        assert_eq!(&maps[start..start + 13], &[0; 13]);
        maps[start + 5 + 10 / 8] |= 1 << (10 % 8);
    }
    let mut page = vec![0; PAGE];
    page[..2].copy_from_slice(&[1, 1]);
    put16(&mut page, 2, (PAGE - 16 - body.len()) as u16);
    page[4..8].copy_from_slice(b"LVAL");
    put16(&mut page, 12, 1);
    put16(&mut page, 14, (PAGE - body.len()) as u16);
    page[PAGE - body.len()..].copy_from_slice(&body);
    after.extend(page);
    after
}

//! Independent populated keyless table images with fresh multi-page DATA roles.
use super::extend_global_map;
use crate::fixtures::access::{
    canonical_creation, canonical_native17_creation, packed_record::append_record, put16, put32,
};

const PAGE: usize = 4096;
const BITMAP_BITS: usize = (PAGE - 4) * 8;

struct Column {
    name: &'static str,
    native: u8,
    width: u16,
    variable: Option<u16>,
}

const NUMBER: [Column; 1] = [Column {
    name: "CopiedScore",
    native: 4,
    width: 4,
    variable: None,
}];
const BOOLEAN: [Column; 1] = [Column {
    name: "Enabled",
    native: 1,
    width: 0,
    variable: None,
}];
const EXACT: [Column; 2] = [
    Column {
        name: "Label",
        native: 10,
        width: 14,
        variable: Some(0),
    },
    Column {
        name: "Amount",
        native: 16,
        width: 17,
        variable: Some(1),
    },
];

fn word(bytes: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()))
}

/// Manual nullable LONG, with no generated field, primary or property body.
pub fn numbers(before: &[u8], name: &str, values: &[Option<i32>]) -> Vec<u8> {
    let records = values
        .iter()
        .map(|value| {
            let mut row = vec![1, 0];
            row.extend(value.unwrap_or(0).to_le_bytes());
            row.push(u8::from(value.is_some()));
            row
        })
        .collect::<Vec<_>>();
    append(before, name, &NUMBER, &records, 7)
}

/// Boolean native0 stores its false/true value in bit0, never a NULL marker.
pub fn booleans(before: &[u8], name: &str, values: &[bool]) -> Vec<u8> {
    let records = values
        .iter()
        .map(|value| vec![1, 0, u8::from(*value)])
        .collect::<Vec<_>>();
    append(before, name, &BOOLEAN, &records, 3)
}

/// Seven native UTF-16 units and Decimal28,4, with NULL distinct from empty Text.
pub fn text_decimals(
    before: &[u8],
    name: &str,
    values: &[(Option<&str>, Option<i128>)],
) -> Vec<u8> {
    let records = values
        .iter()
        .map(|(text, decimal)| {
            let mut row = vec![2, 0];
            if let Some(text) = text {
                assert!(!text.starts_with('\u{feff}'));
                assert!(text.encode_utf16().count() <= 7);
                row.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
            }
            let decimal_start = row.len();
            if let Some(coefficient) = decimal {
                assert!(coefficient.unsigned_abs() < 10_u128.pow(28));
                row.push(if *coefficient < 0 { 128 } else { 0 });
                for shift in [96, 64, 32, 0] {
                    row.extend(((coefficient.unsigned_abs() >> shift) as u32).to_le_bytes());
                }
            }
            let eod = row.len();
            for offset in [eod, decimal_start, 2, 2] {
                row.extend((offset as u16).to_le_bytes());
            }
            row.push(u8::from(text.is_some()) | (u8::from(decimal.is_some()) << 1));
            row
        })
        .collect::<Vec<_>>();
    append(before, name, &EXACT, &records, 11)
}

fn definition(columns: &[Column], count: usize, owned: u32, candidate: u32) -> Vec<u8> {
    let mut bytes = vec![0; PAGE];
    bytes[..2].copy_from_slice(&[2, 1]);
    put32(&mut bytes, 12, 1625);
    put32(&mut bytes, 16, count as u32);
    bytes[24] = 1;
    bytes[40] = 78;
    put16(&mut bytes, 41, columns.len() as u16);
    put16(
        &mut bytes,
        43,
        columns
            .iter()
            .filter(|column| column.variable.is_some())
            .count() as u16,
    );
    put16(&mut bytes, 45, columns.len() as u16);
    put32(&mut bytes, 55, owned);
    put32(&mut bytes, 59, candidate);
    let mut end = 63;
    let mut fixed_offset = 0;
    for (index, column) in columns.iter().enumerate() {
        bytes[end] = column.native;
        put32(&mut bytes, end + 1, 1625);
        put16(&mut bytes, end + 5, index as u16);
        put16(&mut bytes, end + 7, column.variable.unwrap_or(0));
        put16(&mut bytes, end + 9, index as u16);
        if column.native == 10 {
            bytes[end + 11..end + 15].copy_from_slice(&[9, 4, 0, 1]);
        }
        if column.native == 16 {
            bytes[end + 11..end + 13].copy_from_slice(&[28, 4]);
        }
        bytes[end + 15] = if column.variable.is_some() { 2 } else { 3 };
        put16(
            &mut bytes,
            end + 21,
            if column.variable.is_some() {
                0
            } else {
                fixed_offset
            },
        );
        put16(&mut bytes, end + 23, column.width);
        if column.variable.is_none() {
            fixed_offset += column.width;
        }
        end += 25;
    }
    for column in columns {
        let units = column.name.encode_utf16().collect::<Vec<_>>();
        put16(&mut bytes, end, (units.len() * 2) as u16);
        end += 2;
        for unit in units {
            put16(&mut bytes, end, unit);
            end += 2;
        }
    }
    put16(&mut bytes, end, 65535);
    end += 2;
    put32(&mut bytes, 8, end as u32);
    put16(&mut bytes, 2, (PAGE - end - 8) as u16);
    bytes
}

fn map(members: &[usize], next: &mut usize, bitmaps: &mut Vec<Vec<u8>>) -> Vec<u8> {
    let base = members.first().copied().unwrap_or(0) / 64 * 64;
    if members.last().is_none_or(|last| last - base < 64) {
        let mut bytes = vec![0; 13];
        put32(&mut bytes, 1, base as u32);
        for &member in members {
            bytes[5 + (member - base) / 8] |= 1 << ((member - base) % 8);
        }
        return bytes;
    }
    let slots = (members.last().unwrap() / BITMAP_BITS + 1).max(3);
    let mut bytes = vec![0; 1 + 4 * slots];
    bytes[0] = 1;
    for block in 0..slots {
        let subset = members
            .iter()
            .copied()
            .filter(|member| member / BITMAP_BITS == block)
            .collect::<Vec<_>>();
        if subset.is_empty() {
            continue;
        }
        let mut bitmap = vec![0; PAGE];
        bitmap[..4].copy_from_slice(&[5, 1, 0, 0]);
        for member in subset {
            let bit = member % BITMAP_BITS;
            bitmap[4 + bit / 8] |= 1 << (bit % 8);
        }
        put32(&mut bytes, 1 + 4 * block, *next as u32);
        *next += 1;
        bitmaps.push(bitmap);
    }
    bytes
}

fn append(
    before: &[u8],
    name: &str,
    columns: &[Column],
    records: &[Vec<u8>],
    minimum_row: usize,
) -> Vec<u8> {
    assert_eq!(before.len() % PAGE, 0);
    let root = before.len() / PAGE;
    let mut groups = vec![Vec::new()];
    let mut used = 14;
    for record in records {
        assert!(record.len() + 16 <= PAGE);
        if groups.last().unwrap().len() == 256 || used + record.len() + 2 > PAGE {
            groups.push(Vec::new());
            used = 14;
        }
        groups.last_mut().unwrap().push(record.as_slice());
        used += record.len() + 2;
    }
    let data = groups
        .iter()
        .map(|group| {
            let mut page = vec![0; PAGE];
            page[..2].copy_from_slice(&[1, 1]);
            put32(&mut page, 4, root as u32);
            put16(&mut page, 2, (PAGE - 14) as u16);
            for record in group {
                append_record(&mut page, record);
            }
            page
        })
        .collect::<Vec<_>>();
    let members = (root + 1..root + 1 + data.len()).collect::<Vec<_>>();
    let candidates = data
        .iter()
        .zip(&members)
        .filter_map(|(page, &number)| {
            (word(page, 12) < 256 && word(page, 2) >= minimum_row + 2).then_some(number)
        })
        .collect::<Vec<_>>();
    let mut next = root + 1 + data.len();
    let mut bitmaps = Vec::new();
    let owned = map(&members, &mut next, &mut bitmaps);
    let candidate = map(&candidates, &mut next, &mut bitmaps);
    // NO_CLONE: Independent complete AFTER coexists with its immutable BEFORE;
    // raw fixture mutation never consumes the caller's original source.
    let mut after = before.to_vec();
    let map_slot = word(before, 3 * PAGE + 12);
    assert!(map_slot + 2 <= 256);
    append_record(&mut after[3 * PAGE..4 * PAGE], &owned);
    append_record(&mut after[3 * PAGE..4 * PAGE], &candidate);
    let catalog = canonical_creation::catalog_record(root as u32, 0x0f00_0001, name, 1, 0);
    let catalog = canonical_native17_creation::catalog_row(&catalog);
    append_record(&mut after[4 * PAGE..5 * PAGE], &catalog);
    let live = u32::from_le_bytes(before[2 * PAGE + 16..2 * PAGE + 20].try_into().unwrap());
    put32(&mut after, 2 * PAGE + 16, live + 1);
    after.extend(definition(
        columns,
        records.len(),
        (3 << 8) | map_slot as u32,
        (3 << 8) | (map_slot + 1) as u32,
    ));
    for page in data {
        after.extend(page);
    }
    for bitmap in bitmaps {
        after.extend(bitmap);
    }
    assert_eq!(after.len(), next * PAGE);
    extend_global_map(&mut after, before.len() / PAGE);
    after
}

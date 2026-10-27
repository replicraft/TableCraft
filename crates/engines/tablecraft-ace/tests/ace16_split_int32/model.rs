//! Original literal fixed-row/key/page model; no production encoding helpers.
use super::{engine, index_fixture, support};
use engine::{
    Ace16WritePreview, EditError, IndexCheckStatus, Int32Update, NativeImage, check_table,
    read_table, read_table_index,
};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

pub const PAGE: usize = 4096;
pub const ROOT: usize = 13;
pub const RIGHT: usize = 16;
pub const LEFT: usize = 17;
pub const ENTRY_START: usize = 480;
pub const MASK_START: usize = 27;
pub const ROWS: usize = 500;
pub const HISTORY: u32 = 7777;

pub fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

pub fn capture(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

pub fn source(code: u8, descending: bool, alias: bool) -> Vec<u8> {
    let mut bytes = index_fixture::database(descending, alias, false);
    bytes.resize(16 * PAGE, 0x69);
    bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    bytes[20] = code;
    support::put32(&mut bytes, 10 * PAGE + 16, ROWS as u32);
    support::put32(&mut bytes, 10 * PAGE + 67, HISTORY);
    for (number, start, end) in [(12, 0, 250), (15, 250, ROWS)] {
        let rows = (start..end)
            .map(|row| {
                let value = row as i32 + 1;
                let mut record = vec![2, 0];
                record.extend(value.to_le_bytes());
                record.extend(value.to_le_bytes());
                record.push(3);
                (0, record)
            })
            .collect::<Vec<_>>();
        let free = PAGE
            - 14
            - size_of::<u16>() * rows.len()
            - rows.iter().map(|(_, record)| record.len()).sum::<usize>();
        let mut page = support::data_page(10, &rows);
        support::put16(&mut page, 2, free as u16);
        bytes[number * PAGE..(number + 1) * PAGE].copy_from_slice(&page);
    }
    let mut data = vec![0; 69];
    data[6] = 0x90;
    bytes[11 * PAGE..12 * PAGE].copy_from_slice(&support::data_page(0, &[(0, data)]));
    let mut owned = vec![0; 69];
    owned[6] = 0x20;
    bytes[14 * PAGE..15 * PAGE].copy_from_slice(&support::data_page(0, &[(0, owned)]));
    let mut global = vec![0; 69];
    for number in RIGHT..512 {
        global[5 + number / 8] |= 1 << (number % 8);
    }
    bytes[PAGE..2 * PAGE].copy_from_slice(&support::data_page(1, &[(0, global)]));
    let keys = keys(descending, None);
    let page = &mut bytes[ROOT * PAGE..(ROOT + 1) * PAGE];
    page.fill(0xa7);
    page[..ENTRY_START].fill(0);
    page[..2].copy_from_slice(&[4, 1]);
    support::put32(page, 4, 10);
    support::put16(page, 24, 3);
    let mut end = 0;
    for (position, entry) in keys.iter().enumerate() {
        let shared = if position == 0 { 0 } else { 3 };
        page[ENTRY_START + end..ENTRY_START + end + entry.len() - shared]
            .copy_from_slice(&entry[shared..]);
        end += entry.len() - shared;
        page[MASK_START + end / 8] |= 1 << (end % 8);
    }
    support::put16(page, 2, (PAGE - ENTRY_START - end) as u16);
    bytes
}

pub fn keys(descending: bool, replacement: Option<i32>) -> Vec<Vec<u8>> {
    let mut values = (0..ROWS)
        .map(|row| {
            (
                if row == 0 {
                    replacement.unwrap_or(1)
                } else {
                    row as i32 + 1
                },
                row,
            )
        })
        .collect::<Vec<_>>();
    values.sort_by(|(a, row_a), (b, row_b)| {
        (if descending { b.cmp(a) } else { a.cmp(b) }).then(row_a.cmp(row_b))
    });
    values
        .into_iter()
        .map(|(value, row)| {
            let mut encoded = vec![if descending { 0x80 } else { 0x7f }];
            encoded.extend(
                ((value as u32) ^ 0x80000000)
                    .to_be_bytes()
                    .map(|byte| if descending { !byte } else { byte }),
            );
            let page = if row < 250 { 12_u32 } else { 15_u32 };
            let slot = (row % 250) as u32;
            encoded.extend(((page << 8) | slot).to_be_bytes());
            encoded
        })
        .collect()
}

pub fn entries(bytes: &[u8], number: usize) -> Vec<Vec<u8>> {
    let page = &bytes[number * PAGE..(number + 1) * PAGE];
    let prefix = word(page, 24) as usize;
    let mut result: Vec<Vec<u8>> = Vec::new();
    let mut last = 0;
    for bit in 1..PAGE - ENTRY_START + 1 {
        if page[MASK_START + bit / 8] & (1 << (bit % 8)) != 0 {
            let mut entry = if result.is_empty() {
                Vec::new()
            } else {
                result[0][..prefix].to_vec()
            };
            entry.extend_from_slice(&page[ENTRY_START + last..ENTRY_START + bit]);
            result.push(entry);
            last = bit;
        }
    }
    result
}

pub fn word(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
}

pub fn number(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

pub fn bitmap_offset(bytes: &[u8], page: usize, bit: usize) -> usize {
    let start = word(bytes, page * PAGE + 14) as usize;
    page * PAGE + start + 5 + bit / 8
}

pub fn preview(bytes: &[u8], updates: &[Int32Update]) -> Result<Ace16WritePreview, EditError> {
    capture(bytes).preview_ace16_split_int32_updates(
        "Items",
        "Score",
        updates,
        &mut budget(),
        &|| false,
    )
}

pub fn assert_checked(bytes: &[u8]) {
    let report = check_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(
        report
            .indexes()
            .iter()
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );
}

pub fn rows(bytes: &[u8]) -> Vec<Vec<Value>> {
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

pub fn index_rows(bytes: &[u8], label: &str) -> Vec<Vec<Value>> {
    read_table_index(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        label,
        &mut budget(),
        &|| false,
    )
    .unwrap()
    .rows()
    .to_vec()
}

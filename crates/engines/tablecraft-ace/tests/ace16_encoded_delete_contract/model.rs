//! Independent stored records and physical-key goldens; no production encoders.
use super::{fixture, index_fixture, support};
use std::io::Cursor;
use tablecraft_ace::{
    Ace16DeletePreview, EditError, NativeImage, RowDelete, check_table, read_table_index,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

pub struct Model {
    pub bytes: Vec<u8>,
    pub records: Vec<Vec<u8>>,
    pub keys: Vec<Vec<u8>>,
    pub order: Vec<usize>,
}

pub fn budget() -> ResourceBudget {
    ResourceBudget::new(128 * 1024 * 1024, 100_000, 30_000_000)
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

pub fn requests(model: &Model, deleted: &[usize]) -> Vec<RowDelete> {
    deleted
        .iter()
        .map(|&row| RowDelete::new(row, model.records[row].clone()))
        .collect()
}

pub fn candidate(model: &Model, deleted: &[usize]) -> Result<Ace16DeletePreview, EditError> {
    capture(&model.bytes).preview_ace16_deletes(
        "Items",
        &requests(model, deleted),
        &mut budget(),
        &|| false,
    )
}

pub fn wire(model: &Model, row: usize) -> Vec<u8> {
    let mut bytes = model.keys[row].clone();
    bytes.extend(((12_u32 << 8) | row as u32).to_be_bytes());
    bytes
}

pub fn page(height: u8, entries: &[Vec<u8>], previous: u32, next: u32, tail: u32) -> Vec<u8> {
    let mut page = vec![0xa7; 4096];
    page[..480].fill(0);
    page[..2].copy_from_slice(&[if height == 0 { 4 } else { 3 }, 1]);
    page[26] = height;
    support::put32(&mut page, 4, 10);
    support::put32(&mut page, 12, previous);
    support::put32(&mut page, 16, next);
    support::put32(&mut page, 20, tail);
    let mut length = 0;
    for entry in entries {
        page[480 + length..480 + length + entry.len()].copy_from_slice(entry);
        length += entry.len();
        page[27 + length / 8] |= 1 << (length % 8);
    }
    support::put16(&mut page, 2, (3616 - length) as u16);
    page
}

fn database(
    values: Vec<Option<Vec<u8>>>,
    keys: Vec<Vec<u8>>,
    order: Vec<usize>,
    kind: u8,
    family: u8,
    descending: bool,
    alias: bool,
) -> Model {
    let mut bytes = index_fixture::database(descending, alias, true);
    bytes.resize(20 * 4096, 0);
    bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    bytes[20] = 6;
    let metadata = fixture::definition(
        &[
            ("Id", 4, 0, 4),
            ("Score", kind, 0, if kind == 10 { 510 } else { 255 }),
        ],
        values.len() as u32,
        11 << 8,
    );
    bytes[10 * 4096 + 100..10 * 4096 + 125].copy_from_slice(&metadata[88..113]);
    if kind == 10 {
        bytes[10 * 4096 + 111..10 * 4096 + 115].copy_from_slice(&[9, 4, 0, family]);
    }
    if kind == 9 {
        // The reused old helper treats Binary as fixed. These newly authored
        // records deliberately use the variable field and its offset directory.
        bytes[10 * 4096 + 115] = 0;
    }
    support::put16(&mut bytes, 10 * 4096 + 43, 1);
    support::put32(&mut bytes, 10 * 4096 + 16, values.len() as u32);
    support::put32(&mut bytes, 10 * 4096 + 67, values.len() as u32);
    bytes[10 * 4096 + 189] = 0x80;
    bytes[10 * 4096 + 195 + 23] = 0;
    if alias {
        bytes[10 * 4096 + 195 + 28 + 23] = 0;
    }
    let records = values
        .iter()
        .enumerate()
        .map(|(row, value)| {
            let mut bytes = vec![2, 0];
            bytes.extend((row as i32 + 1).to_le_bytes());
            if let Some(value) = value {
                bytes.extend(value);
            }
            let end = bytes.len() as u16;
            bytes.extend(end.to_le_bytes());
            bytes.extend(6_u16.to_le_bytes());
            bytes.extend(1_u16.to_le_bytes());
            bytes.push(if value.is_some() { 3 } else { 1 });
            bytes
        })
        .collect::<Vec<_>>();
    let data = records
        .iter()
        .cloned()
        .map(|row| (0, row))
        .collect::<Vec<_>>();
    bytes[12 * 4096..13 * 4096].copy_from_slice(&support::data_page(10, &data));
    support::put32(&mut bytes, 11 * 4096 + 4, 0);
    bytes[14 * 4096..15 * 4096].copy_from_slice(&support::data_page(
        0,
        &[(0, vec![0, 0, 0, 0, 0, 0, 0x20, 0])],
    ));
    let mut global = vec![0; 69];
    for page in 20..512 {
        global[5 + page / 8] |= 1 << (page % 8);
    }
    bytes[4096..8192].copy_from_slice(&support::data_page(1, &[(0, global)]));
    let mut result = Model {
        bytes,
        records,
        keys,
        order,
    };
    let entries = result
        .order
        .iter()
        .map(|&row| wire(&result, row))
        .collect::<Vec<_>>();
    result.bytes[13 * 4096..14 * 4096].copy_from_slice(&page(0, &entries, 0, 0, 0));
    result
}

fn hex(text: &str) -> Vec<u8> {
    text.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

pub fn text(family: u8, descending: bool, alias: bool) -> Model {
    let values = [
        None,
        Some("".to_string()),
        Some("a".to_string()),
        Some("A".to_string()),
        Some("é".to_string()),
        Some("e\u{301}".to_string()),
        Some("ß".to_string()),
        Some("ss".to_string()),
        Some("a".repeat(255)),
    ];
    let short = match (family, descending) {
        (0, false) => ["0100", "4a0100", "51010e00", "6b6b0100"],
        (0, true) => ["feff00", "b5feff00", "aefef1ff00", "9494feff00"],
        (1, false) => ["0100", "0e020100", "0e21010e00", "0e910e910100"],
        (1, true) => ["feff00", "f1fdfeff00", "f1defef1ff00", "f16ef16efeff00"],
        _ => panic!("closed independent General families"),
    };
    // Small goldens are from the independently generated General key corpus.
    // The family1 510-byte tails were independently obtained from reviewed
    // Jackcess IndexData.truncateEntryBytes: asc27a5, descf75b (26Oct2026).
    let keys = values
        .iter()
        .enumerate()
        .map(|(row, value)| {
            if value.is_none() {
                return vec![if descending { 255 } else { 0 }];
            }
            let mut key = vec![if descending { 128 } else { 127 }];
            if row == 8 {
                if family == 0 {
                    key.extend(std::iter::repeat_n(
                        if descending { 0xb5 } else { 0x4a },
                        255,
                    ));
                    key.extend(hex(short[0]));
                } else {
                    for _ in 0..255 {
                        key.extend(if descending { [0xf1, 0xfd] } else { [0x0e, 2] });
                    }
                    key.truncate(508);
                    key.extend(if descending {
                        [0xf7, 0x5b]
                    } else {
                        [0x27, 0xa5]
                    });
                }
            } else {
                key.extend(hex(short[match row {
                    1 => 0,
                    2 | 3 => 1,
                    4 | 5 => 2,
                    6 | 7 => 3,
                    _ => unreachable!(),
                }]));
            }
            key
        })
        .collect();
    let order = if descending {
        vec![6, 7, 4, 5, 8, 2, 3, 1, 0]
    } else {
        vec![0, 1, 2, 3, 8, 4, 5, 6, 7]
    };
    let values = values
        .into_iter()
        .map(|value| value.map(|text| text.encode_utf16().flat_map(u16::to_le_bytes).collect()))
        .collect();
    database(values, keys, order, 10, family, descending, alias)
}

pub fn binary(descending: bool, alias: bool) -> Model {
    let values = vec![
        None,
        Some(vec![]),
        Some(vec![0]),
        Some(vec![0, 0]),
        Some((1..=8).collect()),
        Some((1..=9).collect()),
        Some(vec![255; 8]),
        Some(vec![255; 9]),
        Some(vec![128, 0, 255]),
    ];
    let keys = values
        .iter()
        .map(|value| {
            let Some(value) = value else {
                return vec![if descending { 255 } else { 0 }];
            };
            let mut key = vec![if descending { 128 } else { 127 }];
            for (index, chunk) in value.chunks(8).enumerate() {
                let mut segment = [if descending { 255 } else { 0 }; 9];
                for (to, byte) in segment.iter_mut().zip(chunk) {
                    *to = if descending { !byte } else { *byte };
                }
                segment[8] = if (index + 1) * 8 < value.len() {
                    9
                } else if descending {
                    !(chunk.len() as u8)
                } else {
                    chunk.len() as u8
                };
                key.extend(segment);
            }
            key
        })
        .collect();
    // Existing Binary profile keeps empty before nonempty even when descending.
    let order = if descending {
        vec![1, 7, 6, 8, 5, 4, 3, 2, 0]
    } else {
        vec![0, 1, 2, 3, 4, 5, 8, 6, 7]
    };
    database(values, keys, order, 9, 0, descending, alias)
}

pub fn entries(bytes: &[u8], number: usize) -> Vec<Vec<u8>> {
    let page = &bytes[number * 4096..(number + 1) * 4096];
    let prefix = u16::from_le_bytes(page[24..26].try_into().unwrap()) as usize;
    let mut first = Vec::new();
    let mut previous = 0;
    let mut result = Vec::new();
    for end in page[27..480].iter().enumerate().flat_map(|(byte, mask)| {
        (0..8).filter_map(move |bit| (mask & (1 << bit) != 0).then_some(byte * 8 + bit))
    }) {
        let fragment = &page[480 + previous..480 + end];
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

pub fn tree(mut model: Model, bounded: bool) -> Model {
    let leaves = [15_u32, 16, 19];
    let mut bounds = Vec::new();
    for (index, rows) in model.order.chunks(3).enumerate() {
        let packets = rows
            .iter()
            .map(|&row| wire(&model, row))
            .collect::<Vec<_>>();
        let number = leaves[index] as usize;
        model.bytes[number * 4096..(number + 1) * 4096].copy_from_slice(&page(
            0,
            &packets,
            if index == 0 { 0 } else { leaves[index - 1] },
            leaves.get(index + 1).copied().unwrap_or(0),
            0,
        ));
        if bounded || index < 2 {
            let mut last = packets.last().unwrap().clone();
            last.extend(leaves[index].to_be_bytes());
            bounds.push(last);
        }
    }
    model.bytes[13 * 4096..14 * 4096].copy_from_slice(&page(
        1,
        &bounds,
        0,
        0,
        if bounded { 0 } else { 19 },
    ));
    model.bytes[14 * 4096..15 * 4096].copy_from_slice(&support::data_page(
        0,
        &[(0, vec![0, 0, 0, 0, 0, 0, 0xa0, 9])],
    ));
    model
}

pub fn verify_rows(model: &Model, after: &[u8], deleted: &[usize]) {
    let expected = model
        .order
        .iter()
        .filter(|row| !deleted.contains(row))
        .map(|row| Value::Int32(*row as i32 + 1))
        .collect::<Vec<_>>();
    for name in if model.bytes[10 * 4096 + 47] == 2 {
        vec!["ScoreKey", "ScoreAlias"]
    } else {
        vec!["ScoreKey"]
    } {
        let read = read_table_index(
            &mut Cursor::new(after),
            CatalogScope::new(1),
            "Items",
            name,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            read.rows()
                .iter()
                .map(|row| row[0].clone())
                .collect::<Vec<_>>(),
            expected
        );
    }
    check_table(
        &mut Cursor::new(after),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
}

pub fn single_overflow() -> Model {
    let mut model = text(1, false, false);
    model.records = vec![model.records[8].clone()];
    model.keys = vec![model.keys[8].clone()];
    model.order = vec![0];
    support::put32(&mut model.bytes, 10 * 4096 + 16, 1);
    model.bytes[10 * 4096 + 189] = 0x81;
    model.bytes[10 * 4096 + 195 + 23] = 1;
    model.bytes[12 * 4096..13 * 4096]
        .copy_from_slice(&support::data_page(10, &[(0, model.records[0].clone())]));
    let entry = wire(&model, 0);
    model.bytes[13 * 4096..14 * 4096].copy_from_slice(&page(0, &[entry], 0, 0, 0));
    model
}

pub fn null_memo() -> Model {
    let mut model = text(1, false, false);
    model.records = vec![model.records[0].clone()];
    model.keys = vec![model.keys[0].clone()];
    model.order = vec![0];
    support::put32(&mut model.bytes, 10 * 4096 + 16, 1);
    model.bytes[10 * 4096 + 100] = 12;
    support::put16(&mut model.bytes, 10 * 4096 + 123, 0);
    model.bytes[12 * 4096..13 * 4096]
        .copy_from_slice(&support::data_page(10, &[(0, model.records[0].clone())]));
    let entry = wire(&model, 0);
    model.bytes[13 * 4096..14 * 4096].copy_from_slice(&page(0, &[entry], 0, 0, 0));
    model
}

pub fn composite() -> Model {
    let mut model = text(1, false, false);
    model.records.truncate(8);
    model.keys.truncate(8);
    model.order = (0..8).collect();
    support::put32(&mut model.bytes, 10 * 4096 + 16, 8);
    support::put16(&mut model.bytes, 10 * 4096 + 150, 0);
    model.bytes[10 * 4096 + 152] = 1;
    for (row, key) in model.keys.iter_mut().enumerate() {
        key.push(127);
        key.extend((0x80000000_u32 ^ (row as u32 + 1)).to_be_bytes());
    }
    let rows = model
        .records
        .iter()
        .cloned()
        .map(|record| (0, record))
        .collect::<Vec<_>>();
    model.bytes[12 * 4096..13 * 4096].copy_from_slice(&support::data_page(10, &rows));
    let entries = model
        .order
        .iter()
        .map(|&row| wire(&model, row))
        .collect::<Vec<_>>();
    model.bytes[13 * 4096..14 * 4096].copy_from_slice(&page(0, &entries, 0, 0, 0));
    model
}

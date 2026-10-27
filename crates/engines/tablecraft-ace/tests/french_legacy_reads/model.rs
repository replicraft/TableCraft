//! Literal French wire goldens from the pinned public-writer original.
use super::{fixture, index_fixture, support};
use tablecraft_core::limits::ResourceBudget;

pub struct Model {
    pub bytes: Vec<u8>,
    pub records: Vec<Vec<u8>>,
    pub keys: Vec<Vec<u8>>,
    pub order: Vec<usize>,
}

pub fn budget() -> ResourceBudget {
    ResourceBudget::new(128 * 1024 * 1024, 100_000, 30_000_000)
}

#[cfg(feature = "ace16-write-preview")]
pub fn capture(bytes: &[u8]) -> tablecraft_ace::NativeImage {
    tablecraft_ace::NativeImage::capture(
        &mut std::io::Cursor::new(bytes),
        tablecraft_model::CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

#[cfg(feature = "ace16-write-preview")]
pub fn candidate(
    model: &Model,
    deleted: &[usize],
) -> Result<tablecraft_ace::Ace16DeletePreview, tablecraft_ace::EditError> {
    let requests = deleted
        .iter()
        .map(|&row| tablecraft_ace::RowDelete::new(row, model.records[row].clone()))
        .collect::<Vec<_>>();
    capture(&model.bytes).preview_ace16_deletes("Items", &requests, &mut budget(), &|| false)
}

fn hex(text: &str) -> Vec<u8> {
    text.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

pub fn database(code: u8, descending: bool, node: Option<bool>) -> Model {
    let mut bytes = index_fixture::database(descending, true, true);
    bytes.resize(20 * 4096, 0);
    bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    let metadata = fixture::definition(&[("Id", 4, 0, 4), ("Score", 10, 0, 510)], 9, 11 << 8);
    bytes[10 * 4096 + 100..10 * 4096 + 125].copy_from_slice(&metadata[88..113]);
    support::put16(&mut bytes, 10 * 4096 + 43, 1);
    support::put32(&mut bytes, 10 * 4096 + 16, 9);
    support::put32(&mut bytes, 10 * 4096 + 67, 9);
    bytes[10 * 4096 + 189] = 0x80;
    bytes[10 * 4096 + 195 + 23] = 0;
    bytes[10 * 4096 + 195 + 28 + 23] = 0;
    support::put32(&mut bytes, 11 * 4096 + 4, 0);
    bytes[14 * 4096..15 * 4096].copy_from_slice(&support::data_page(
        0,
        &[(0, vec![0, 0, 0, 0, 0, 0, 0x20, 0])],
    ));
    let mut model = Model {
        bytes,
        records: Vec::new(),
        keys: Vec::new(),
        order: Vec::new(),
    };
    model.bytes[20] = code;
    model.bytes[10 * 4096 + 111..10 * 4096 + 115].copy_from_slice(&[12, 4, 0, 0]);
    let values = [
        None,
        Some(""),
        Some("cote"),
        Some("coté"),
        Some("côte"),
        Some("côté"),
        Some("COTE"),
        Some("cote"),
        Some("côté"),
    ];
    let golden = if descending {
        [
            "ff",
            "80feff00",
            "80b29b92aefeff00",
            "80b29b92aefef1ff00",
            "80b29b92aefefdfdedff00",
            "80b29b92aefef1fdedff00",
            "80b29b92aefeff00",
            "80b29b92aefeff00",
            "80b29b92aefef1fdedff00",
        ]
    } else {
        [
            "00",
            "7f0100",
            "7f4d646d510100",
            "7f4d646d51010e00",
            "7f4d646d510102021200",
            "7f4d646d51010e021200",
            "7f4d646d510100",
            "7f4d646d510100",
            "7f4d646d51010e021200",
        ]
    };
    model.keys = golden.into_iter().map(hex).collect();
    model.order = if descending {
        vec![5, 8, 3, 4, 2, 6, 7, 1, 0]
    } else {
        vec![0, 1, 2, 6, 7, 4, 3, 5, 8]
    };
    model.records = values
        .iter()
        .enumerate()
        .map(|(position, value)| {
            let mut record = vec![2, 0];
            record.extend((position as i32 + 1).to_le_bytes());
            if let Some(value) = value {
                record.extend(value.encode_utf16().flat_map(u16::to_le_bytes));
            }
            let end = record.len() as u16;
            record.extend(end.to_le_bytes());
            record.extend(6_u16.to_le_bytes());
            record.extend(1_u16.to_le_bytes());
            record.push(if value.is_some() { 3 } else { 1 });
            record
        })
        .collect();
    let rows = model
        .records
        .iter()
        .cloned()
        .map(|row| (0, row))
        .collect::<Vec<_>>();
    model.bytes[12 * 4096..13 * 4096].copy_from_slice(&support::data_page(10, &rows));
    let entries = model
        .order
        .iter()
        .map(|&row| wire(&model, row))
        .collect::<Vec<_>>();
    model.bytes[13 * 4096..14 * 4096].copy_from_slice(&page(0, &entries, 0, 0, 0));
    if let Some(bounded) = node {
        model = tree(model, bounded);
    }
    model
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

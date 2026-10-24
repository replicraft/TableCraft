//! Original synthetic stored-history records shared by engine and adapter tests.
use super::{
    base, complex_fixture,
    support::{data_page, put16, put32},
};

const PAGE_BYTES: usize = 4096;
const FIELD_START: usize = 63;
const FIELD_BYTES: usize = 25;
const TEMPLATE_PAGE: usize = 17;
const FLAT_PAGE: usize = 20;
const DATA_PAGE: usize = 22;

type Record<'a> = (i32, i32, Option<&'a str>, Option<u64>);

pub fn definition(fields: &[(&str, u8, u16, u16)], rows: u32, usage: u32) -> Vec<u8> {
    let mut page = base::definition(fields, rows, usage);
    let mut variable = 0;
    for (i, (_, kind, _, _)) in fields.iter().enumerate() {
        if *kind == 12 {
            let at = FIELD_START + i * FIELD_BYTES;
            page[at + 15] = 2;
            put16(&mut page, at + 7, variable);
            variable += 1;
        }
    }
    put16(&mut page, 43, variable);
    page
}
pub fn database_with(records: &[Record<'_>]) -> Vec<u8> {
    let mut bytes = complex_fixture::with_mappings(vec![complex_fixture::mapping(7, 10, 20, 17)]);
    let mut owner = definition(
        &[("Id", 4, 0, 4), ("Score", 0x12, 4, 4), ("Notes", 12, 0, 0)],
        3,
        11 << 8,
    );
    put32(&mut owner, FIELD_START + FIELD_BYTES + 11, 7);
    owner[FIELD_START + FIELD_BYTES + 15] = 7;
    owner[FIELD_START + FIELD_BYTES + 16] = 0x20;
    bytes[10 * PAGE_BYTES..11 * PAGE_BYTES].copy_from_slice(&owner);
    let parents = [10_i32, 20, 30]
        .into_iter()
        .enumerate()
        .map(|(i, key)| {
            let mut row = 3_u16.to_le_bytes().to_vec();
            row.extend((i as i32 + 1).to_le_bytes());
            row.extend(key.to_le_bytes());
            row.extend(10_u16.to_le_bytes());
            row.extend(10_u16.to_le_bytes());
            row.extend(1_u16.to_le_bytes());
            row.push(3);
            (0, row)
        })
        .collect::<Vec<_>>();
    bytes[12 * PAGE_BYTES..13 * PAGE_BYTES].copy_from_slice(&data_page(10, &parents));
    bytes[TEMPLATE_PAGE * PAGE_BYTES..(TEMPLATE_PAGE + 1) * PAGE_BYTES].copy_from_slice(
        &definition(&[("Notes", 12, 0, 0), ("Modified", 8, 0, 8)], 0, 18 << 8),
    );
    let mut flat = definition(
        &[
            ("Owner", 4, 0, 4),
            ("Notes", 12, 0, 0),
            ("Modified", 8, 4, 8),
            ("ChildID", 4, 12, 4),
        ],
        records.len() as u32,
        21 << 8,
    );
    flat[FIELD_START + 16] = 8;
    flat[FIELD_START + 3 * FIELD_BYTES + 15] = 7;
    bytes[FLAT_PAGE * PAGE_BYTES..(FLAT_PAGE + 1) * PAGE_BYTES].copy_from_slice(&flat);
    let rows = records
        .iter()
        .map(|&(key, id, value, modified)| {
            let mut row = 4_u16.to_le_bytes().to_vec();
            row.extend(key.to_le_bytes());
            row.extend(modified.unwrap_or(0).to_le_bytes());
            row.extend(id.to_le_bytes());
            if let Some(value) = value {
                let text = value
                    .encode_utf16()
                    .flat_map(u16::to_le_bytes)
                    .collect::<Vec<_>>();
                row.extend((0x8000_0000 | text.len() as u32).to_le_bytes());
                row.extend([0; 8]);
                row.extend(text);
            }
            let end = row.len() as u16;
            row.extend(end.to_le_bytes());
            row.extend(18_u16.to_le_bytes());
            row.extend(1_u16.to_le_bytes());
            row.push(9 | u8::from(value.is_some()) << 1 | u8::from(modified.is_some()) << 2);
            (0, row)
        })
        .collect::<Vec<_>>();
    bytes[DATA_PAGE * PAGE_BYTES..(DATA_PAGE + 1) * PAGE_BYTES]
        .copy_from_slice(&data_page(FLAT_PAGE as u32, &rows));
    bytes
}
pub fn database() -> Vec<u8> {
    database_with(&[
        (10, 2, Some("é🦀\0older"), Some((-0.25_f64).to_bits())),
        (10, 9, Some("newer"), Some(0.25_f64.to_bits())),
        (20, 7, Some(""), Some(1)),
        (20, 3, None, None),
    ])
}

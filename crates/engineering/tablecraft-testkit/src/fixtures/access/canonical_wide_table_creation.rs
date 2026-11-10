//! Independent 255-column initial table, chained definition and external MR2.
//! No native creator, schema/property decoder or mutation planner builds bytes.
pub mod appended;
pub mod rows;
use super::{
    canonical_blank_creation, canonical_created_default_zero, canonical_creation,
    canonical_creation_properties, canonical_native17_creation, packed_record::append_record,
    put16, put32,
};

const PAGE: usize = 4096;
const FIELD_COUNT: usize = 255;
const DEFINITION: usize = 7;
const DATA: usize = 10;
const INDEX: usize = 11;
const PROPERTY_FIRST: usize = 12;
const PROPERTY_CHUNK: usize = 4072;

pub struct Source {
    pub before: Vec<u8>,
    pub after: Vec<u8>,
    pub definition_pages: Vec<u32>,
    pub data_page: u32,
    pub index_page: u32,
    pub property_pages: Vec<u32>,
    pub property_body: Vec<u8>,
}

/// Literal generated-ID name plus 254 distinct native LONG companions.
pub fn field_name(index: usize) -> String {
    assert!(index < FIELD_COUNT);
    if index == 0 {
        "ID".into()
    } else {
        format!("Field{index}")
    }
}

fn word(bytes: &[u8], at: usize) -> usize {
    u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()) as usize
}

fn name(text: &str) -> Vec<u8> {
    let payload = text
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    let mut bytes = (payload.len() as u16).to_le_bytes().to_vec();
    bytes.extend(payload);
    bytes
}

fn block(kind: u16, payload: &[u8]) -> Vec<u8> {
    let mut bytes = ((6 + payload.len()) as u32).to_le_bytes().to_vec();
    bytes.extend(kind.to_le_bytes());
    bytes.extend(payload);
    bytes
}

fn properties() -> Vec<u8> {
    let mut dictionary = name("Required");
    dictionary.extend(name("DefaultValue"));
    let mut body = b"MR2\0".to_vec();
    body.extend(block(128, &dictionary));
    for index in 1..FIELD_COUNT {
        let label = name(&field_name(index));
        let mut group = ((4 + label.len()) as u32).to_le_bytes().to_vec();
        group.extend(label);
        group.extend([9, 0, 1, 1, 0, 0, 1, 0, 0]); // Required(false).
        group.extend([10, 0, 1, 12, 1, 0, 2, 0, b'0', 0]); // DefaultValue Memo "0".
        body.extend(block(1, &group));
    }
    body
}

fn logical_definition(
    seed: &[u8],
    owned: u32,
    candidate: u32,
    index_map: u32,
    index_page: u32,
) -> Vec<u8> {
    let original = &seed[7 * PAGE..8 * PAGE];
    let old_length = u32::from_le_bytes(original[8..12].try_into().unwrap()) as usize;
    let mut logical = original[..75].to_vec();
    put16(&mut logical, 41, FIELD_COUNT as u16);
    put16(&mut logical, 43, 0);
    put16(&mut logical, 45, FIELD_COUNT as u16);
    put32(&mut logical, 55, owned);
    put32(&mut logical, 59, candidate);
    for index in 0..FIELD_COUNT {
        let mut column = [0; 25];
        column[0] = 4;
        put32(&mut column, 1, 1625);
        put16(&mut column, 5, index as u16);
        put16(&mut column, 9, index as u16);
        column[15] = if index == 0 { 7 } else { 3 };
        put16(&mut column, 21, (4 * index) as u16);
        put16(&mut column, 23, 4);
        logical.extend(column);
    }
    for index in 0..FIELD_COUNT {
        logical.extend(name(&field_name(index)));
    }
    let index_start = logical.len();
    // The independent one-column seed has a 75B indexed header, one 25B
    // declaration and its six-byte ID name before the primary index metadata.
    assert_eq!(&original[100..106], &[4, 0, b'I', 0, b'D', 0]);
    logical.extend_from_slice(&original[106..old_length]);
    put32(&mut logical, index_start + 34, index_map);
    put32(&mut logical, index_start + 38, index_page);
    let length = logical.len();
    put32(&mut logical, 8, length as u32);
    logical
}

fn definition_pages(logical: &[u8], root: usize) -> Vec<Vec<u8>> {
    let mut pages = Vec::new();
    let mut cursor = 0;
    while cursor < logical.len() {
        let mut page = vec![0; PAGE];
        let header = if pages.is_empty() { 0 } else { 8 };
        page[..2].copy_from_slice(&[2, 1]);
        let take = (PAGE - header).min(logical.len() - cursor);
        page[header..header + take].copy_from_slice(&logical[cursor..cursor + take]);
        cursor += take;
        let next = if cursor < logical.len() {
            root + pages.len() + 1
        } else {
            0
        };
        put32(&mut page, 4, next as u32);
        put16(&mut page, 2, PAGE.saturating_sub(header + take + 8) as u16);
        pages.push(page);
    }
    assert_eq!(pages.len(), 3);
    pages
}

fn maps(before: &[u8], property_count: usize) -> Vec<u8> {
    let page = &before[3 * PAGE..4 * PAGE];
    assert_eq!(word(page, 12), 22);
    let mut rows = Vec::new();
    let mut end = PAGE;
    for slot in 0..22 {
        let start = word(page, 14 + 2 * slot);
        rows.push(page[start..end].to_vec());
        end = start;
    }
    for slot in [3, 7] {
        rows[slot][5..].fill(0);
        rows[slot][5 + DATA / 8] |= 1 << (DATA % 8);
    }
    for index in 0..property_count {
        let page = PROPERTY_FIRST + index;
        rows[16][5 + page / 8] |= 1 << (page % 8);
    }
    let last = PROPERTY_FIRST + property_count - 1;
    rows[17][5 + last / 8] |= 1 << (last % 8);
    let mut index = vec![0; 13];
    index[5 + INDEX / 8] |= 1 << (INDEX % 8);
    rows.push(index);
    canonical_native17_creation::packed(0, rows)
}

/// Empty Table1(ID Increment/PrimaryKey, Field1..Field254 nullable LONG/default0).
/// Definitions occupy 7/8/9, DATA10, primary leaf11 and chained catalog LvProp
/// pages12 onward. Only the last partial LVAL page is a candidate for reuse.
pub fn source() -> Source {
    let before = canonical_blank_creation::source();
    let seed =
        canonical_created_default_zero::source(canonical_created_default_zero::Source::IdOnly);
    let logical = logical_definition(
        &seed,
        (3 << 8) | 3,
        (3 << 8) | 7,
        (3 << 8) | 22,
        INDEX as u32,
    );
    let definitions = definition_pages(&logical, DEFINITION);
    let body = properties();
    let chunks = body.chunks(PROPERTY_CHUNK).collect::<Vec<_>>();
    assert_eq!(chunks.len(), 3);
    assert!(chunks.last().unwrap().len() < PROPERTY_CHUNK);
    // NO_CLONE: The complete independent BEFORE remains available for source
    // preservation checks while AFTER receives only its declared page changes.
    let mut after = before.clone();
    after[3 * PAGE..4 * PAGE].copy_from_slice(&maps(&before, chunks.len()));
    let row = canonical_creation::catalog_record(7, 0x0f00_0001, "Table1", 1, 0);
    let row = canonical_native17_creation::catalog_row(&row);
    let mut row = canonical_creation_properties::attach(&row, &[]);
    let descriptor = row.len() - 29 - 12;
    put32(&mut row, descriptor, body.len() as u32);
    put32(&mut row, descriptor + 4, (PROPERTY_FIRST as u32) << 8);
    append_record(&mut after[4 * PAGE..5 * PAGE], &row);
    put32(&mut after, 2 * PAGE + 16, 8);
    for page in definitions {
        after.extend(page);
    }
    let data = canonical_native17_creation::packed(DEFINITION as u32, Vec::new());
    after.extend(data);
    let index = &seed[9 * PAGE..10 * PAGE];
    after.extend(index);
    for (index, chunk) in chunks.iter().enumerate() {
        let next = if index + 1 < chunks.len() {
            ((PROPERTY_FIRST + index + 1) as u32) << 8
        } else {
            0
        };
        let mut record = next.to_le_bytes().to_vec();
        record.extend_from_slice(chunk);
        after.extend(canonical_native17_creation::packed(
            u32::from_le_bytes(*b"LVAL"),
            vec![record],
        ));
    }
    Source {
        before,
        after,
        definition_pages: vec![7, 8, 9],
        data_page: DATA as u32,
        index_page: INDEX as u32,
        property_pages: (PROPERTY_FIRST..PROPERTY_FIRST + chunks.len())
            .map(|page| page as u32)
            .collect(),
        property_body: body,
    }
}

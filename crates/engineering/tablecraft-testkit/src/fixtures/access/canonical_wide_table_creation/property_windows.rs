//! Full-width names/properties cross the first catalog property-map window.
//! The empty table does not assert that its combined defaults fit one record.
use super::{PAGE, PROPERTY_CHUNK, Source, block, definition_pages, name, word};
use crate::fixtures::access::{
    canonical_blank_creation, canonical_created_default_zero, canonical_creation,
    canonical_creation_properties, canonical_native17_creation, packed_record::append_record,
    put16, put32,
};

/// Exactly 64 UTF-16 units, with a unique native name for each companion.
pub fn field_name(index: usize) -> String {
    assert!((1..255).contains(&index));
    format!("Field{index:03}{}", "x".repeat(56))
}

pub fn default_expression() -> String {
    format!("\"{}\"", "d".repeat(253))
}

pub fn description() -> String {
    "e".repeat(255)
}

fn text_property(group: &mut Vec<u8>, key: u16, native_type: u8, value: &str) {
    let payload = value
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    group.extend(((8 + payload.len()) as u16).to_le_bytes());
    group.extend([1, native_type]);
    group.extend(key.to_le_bytes());
    group.extend((payload.len() as u16).to_le_bytes());
    group.extend(payload);
}

fn properties() -> Vec<u8> {
    let mut dictionary = Vec::new();
    for key in ["Required", "AllowZeroLength", "DefaultValue", "Description"] {
        dictionary.extend(name(key));
    }
    let mut body = b"MR2\0".to_vec();
    body.extend(block(128, &dictionary));
    for index in 1..255 {
        let label = name(&field_name(index));
        let mut group = ((4 + label.len()) as u32).to_le_bytes().to_vec();
        group.extend(label);
        group.extend([9, 0, 1, 1, 0, 0, 1, 0, 0]);
        group.extend([9, 0, 1, 1, 1, 0, 1, 0, 255]);
        text_property(&mut group, 2, 12, &default_expression());
        text_property(&mut group, 3, 10, &description());
        body.extend(block(1, &group));
    }
    assert_eq!(body.len(), 303_386);
    body
}

fn definition(seed: &[u8]) -> Vec<u8> {
    let original = &seed[7 * PAGE..8 * PAGE];
    let end = u32::from_le_bytes(original[8..12].try_into().unwrap()) as usize;
    let mut logical = original[..75].to_vec();
    put16(&mut logical, 41, 255);
    put16(&mut logical, 43, 254);
    put16(&mut logical, 45, 255);
    put32(&mut logical, 55, (3 << 8) | 3);
    put32(&mut logical, 59, (3 << 8) | 7);
    for index in 0..255 {
        let mut column = [0; 25];
        column[0] = if index == 0 { 4 } else { 10 };
        put32(&mut column, 1, 1625);
        put16(&mut column, 5, index as u16);
        put16(&mut column, 9, index as u16);
        column[15] = if index == 0 { 7 } else { 2 };
        if index > 0 {
            put16(&mut column, 7, (index - 1) as u16);
            column[11..15].copy_from_slice(&[9, 4, 0, 1]);
        }
        put16(&mut column, 23, if index == 0 { 4 } else { 510 });
        logical.extend(column);
    }
    logical.extend(name("ID"));
    for index in 1..255 {
        logical.extend(name(&field_name(index)));
    }
    let index_start = logical.len();
    logical.extend_from_slice(&original[106..end]);
    put32(&mut logical, index_start + 34, (3 << 8) | 22);
    put32(&mut logical, index_start + 38, 18);
    let length = logical.len();
    assert_eq!(length, 39_580);
    put32(&mut logical, 8, length as u32);
    logical
}

fn maps(before: &[u8]) -> Vec<u8> {
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
        rows[slot][7] = 2; // DATA17.
    }
    rows[16] = vec![1, 94, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    rows[17] = vec![0, 64, 0, 0, 0, 0, 0, 0, 32, 0, 0, 0, 0]; // page93.
    let mut index = vec![0; 13];
    index[7] = 4; // INDEX18.
    rows.push(index);
    canonical_native17_creation::packed(0, rows)
}

/// Empty Table1 with ID plus 254 Text(255) columns and 75 property LVAL pages.
pub fn source() -> Source {
    let before = canonical_blank_creation::source();
    let seed =
        canonical_created_default_zero::source(canonical_created_default_zero::Source::IdOnly);
    let definitions = definition_pages(&definition(&seed), 7);
    assert_eq!(definitions.len(), 10);
    let body = properties();
    let chunks = body.chunks(PROPERTY_CHUNK).collect::<Vec<_>>();
    assert_eq!(chunks.len(), 75);
    // NO_CLONE: Expose immutable BEFORE alongside a separately authored AFTER.
    let mut after = before.clone();
    after[3 * PAGE..4 * PAGE].copy_from_slice(&maps(&before));
    let row = canonical_creation::catalog_record(7, 0x0f00_0001, "Table1", 1, 0);
    let row = canonical_native17_creation::catalog_row(&row);
    let mut row = canonical_creation_properties::attach(&row, &[]);
    let descriptor = row.len() - 29 - 12;
    put32(&mut row, descriptor, body.len() as u32);
    put32(&mut row, descriptor + 4, 19 << 8);
    append_record(&mut after[4 * PAGE..5 * PAGE], &row);
    put32(&mut after, 2 * PAGE + 16, 8);
    for page in definitions {
        after.extend(page);
    }
    after.extend(canonical_native17_creation::packed(7, Vec::new()));
    after.extend_from_slice(&seed[9 * PAGE..10 * PAGE]);
    for (index, chunk) in chunks.iter().enumerate() {
        let next = if index == 74 {
            0
        } else {
            ((20 + index) as u32) << 8
        };
        let mut record = next.to_le_bytes().to_vec();
        record.extend_from_slice(chunk);
        after.extend(canonical_native17_creation::packed(
            u32::from_le_bytes(*b"LVAL"),
            vec![record],
        ));
    }
    let mut bitmap = vec![0; PAGE];
    bitmap[..4].copy_from_slice(&[5, 1, 0, 0]);
    for page in 19..94 {
        bitmap[4 + page / 8] |= 1 << (page % 8);
    }
    after.extend(bitmap);
    // Independently rebuild the single global free-map row at its actual width.
    let free = vec![0; 17];
    after[PAGE..2 * PAGE].copy_from_slice(&canonical_native17_creation::packed(1, vec![free]));
    assert_eq!(after.len(), 95 * PAGE);
    Source {
        before,
        after,
        definition_pages: (7..17).collect(),
        data_page: 17,
        index_page: 18,
        property_pages: (19..94).collect(),
        property_body: body,
    }
}

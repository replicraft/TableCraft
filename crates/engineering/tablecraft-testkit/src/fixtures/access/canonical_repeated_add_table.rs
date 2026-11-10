//! Independently authored populated repeated-table appends, without native writers.
use super::{
    canonical_add_table, canonical_creation, canonical_native17_creation, data_page,
    packed_record::append_record, put16, put32,
};
use tablecraft_core::values::Value;

const PAGE: usize = 4096;
const MIN_EXISTING_TABLES: usize = 2;
const MAX_EXISTING_TABLES: usize = 18;
const OPAQUE_AT: usize = 3500;
const DATA_GAP_AT: usize = 512;

/// Literal native BEFORE and AFTER for the next empty Id/Name(20) table.
pub struct Source {
    pub before: Vec<u8>,
    pub after: Vec<u8>,
    pub table: String,
    pub original_rows: [Vec<Value>; 2],
}

/// Third follows Items(Id=1,Score=17) and More(Id=1,Name="Ada").
pub fn source() -> Source {
    with_existing_tables(2)
}

/// Build later empty tables through the last bitmap-admitted append. Geometry is
/// authored locally; no producer layout, schema codec or mutation supplies bytes.
pub fn with_existing_tables(existing: usize) -> Source {
    assert!((MIN_EXISTING_TABLES..=MAX_EXISTING_TABLES).contains(&existing));
    let first = canonical_add_table::source();
    let mut bytes = first.after;

    // Independently encode one supplied row in More, retaining Increment LAST
    // and primary unique-key history. No native insertion implementation is used.
    put32(&mut bytes, 10 * PAGE + 16, 1);
    put32(&mut bytes, 10 * PAGE + 20, 1);
    put32(&mut bytes, 10 * PAGE + 67, 1);
    let row = vec![2, 0, 1, 0, 0, 0, 65, 0, 100, 0, 97, 0, 12, 0, 6, 0, 1, 0, 3];
    let mut data = data_page(10, &[(0, row)]);
    put16(&mut data, 2, 4061);
    bytes[11 * PAGE..12 * PAGE].copy_from_slice(&data);
    let root = &mut bytes[12 * PAGE..13 * PAGE];
    put16(root, 2, 3607);
    root[28] = 2;
    root[480..489].copy_from_slice(&[127, 128, 0, 0, 1, 0, 0, 11, 0]);
    bytes[10 * PAGE + OPAQUE_AT..10 * PAGE + OPAQUE_AT + 16].copy_from_slice(b"second-td-opaque");
    bytes[11 * PAGE + DATA_GAP_AT..11 * PAGE + DATA_GAP_AT + 16]
        .copy_from_slice(b"second-data-tail");
    bytes[12 * PAGE + OPAQUE_AT..12 * PAGE + OPAQUE_AT + 16].copy_from_slice(b"second-root-tail");

    for number in 3..=existing {
        append_table(&mut bytes, &table_name(number));
    }
    let before = bytes.clone();
    let table = table_name(existing + 1);
    append_table(&mut bytes, &table);
    assert_eq!(before.len(), (7 + 3 * existing) * PAGE);
    assert_eq!(bytes.len(), (7 + 3 * (existing + 1)) * PAGE);
    assert_eq!(&bytes[7 * PAGE..before.len()], &before[7 * PAGE..]);
    Source {
        before,
        after: bytes,
        table,
        original_rows: [
            first.original_row,
            vec![Value::Int32(1), Value::Text("Ada".into())],
        ],
    }
}

/// Nineteen tables occupy all represented bitmap pages; another append refuses.
pub fn capacity_source() -> Vec<u8> {
    with_existing_tables(18).after
}

fn table_name(number: usize) -> String {
    if number == 3 {
        "Third".into()
    } else {
        format!("Table{number}")
    }
}

fn append_table(bytes: &mut Vec<u8>, name: &str) {
    let existing = (bytes.len() / PAGE - 7) / 3;
    assert_eq!(bytes.len(), (7 + 3 * existing) * PAGE);
    let definition_page = 7 + 3 * existing;
    let data_page_number = definition_page + 1;
    let root_page = definition_page + 2;
    let owned_slot = 20 + 3 * existing;
    assert!(root_page < 64);

    let maps = &mut bytes[3 * PAGE..4 * PAGE];
    assert_eq!(
        u16::from_le_bytes(maps[12..14].try_into().unwrap()) as usize,
        owned_slot
    );
    for member in [data_page_number, data_page_number, root_page] {
        let mut map = [0; 13];
        map[5 + member / 8] = 1 << (member % 8);
        append_record(maps, &map);
    }
    let catalog_row =
        canonical_creation::catalog_record(definition_page as u32, 0x0f000001, name, 1, 0);
    append_record(
        &mut bytes[4 * PAGE..5 * PAGE],
        &canonical_native17_creation::catalog_row(&catalog_row),
    );
    put32(bytes, 2 * PAGE + 16, (8 + existing) as u32);

    // The independently authored empty More definition supplies only its raw
    // field/index spelling; relocated references and no row history are explicit.
    let mut definition = canonical_add_table::source().after[10 * PAGE..11 * PAGE].to_vec();
    put32(&mut definition, 55, (3 << 8) | owned_slot as u32);
    put32(&mut definition, 59, (3 << 8) | (owned_slot + 1) as u32);
    let index = 75 + 2 * 25 + 6 + 10;
    put32(
        &mut definition,
        index + 34,
        (3 << 8) | (owned_slot + 2) as u32,
    );
    put32(&mut definition, index + 38, root_page as u32);
    bytes.extend(definition);

    let mut data = data_page(definition_page as u32, &[]);
    put16(&mut data, 2, 4082);
    bytes.extend(data);
    let mut root = vec![0; PAGE];
    root[..2].copy_from_slice(&[4, 1]);
    put16(&mut root, 2, 3616);
    put32(&mut root, 4, definition_page as u32);
    bytes.extend(root);
}

//! Literal desktop Currency declarations and exact scaled fixed-cell changes.
use super::desktop;
use crate::fixtures::access::{
    canonical_created_default_zero, canonical_creation_properties,
    canonical_initial_table_creation::{self as initial, Column},
    generated_record_insert::placement::{self, Pages},
    put16, raw_forwarding_chain,
};
use tablecraft_core::values::{Currency, Value};

const PAGE: usize = 4096;
const COLUMNS: [Column; 2] = [
    Column {
        name: "ID",
        native_type: 4,
        fixed_offset: Some(0),
        width: 4,
        variable_index: 0,
        precision: 0,
        scale: 0,
    },
    Column {
        name: "Amount",
        native_type: 5,
        fixed_offset: Some(4),
        width: 8,
        variable_index: 0,
        precision: 0,
        scale: 0,
    },
];

#[derive(Clone, Copy, Debug)]
pub enum Input {
    Omitted,
    Null,
    Value(i64),
}

fn word(bytes: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()))
}

fn name(value: &str) -> Vec<u8> {
    let payload = value
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    let mut bytes = (payload.len() as u16).to_le_bytes().to_vec();
    bytes.extend(payload);
    bytes
}

fn block(kind: u16, payload: &[u8]) -> Vec<u8> {
    let mut bytes = ((payload.len() + 6) as u32).to_le_bytes().to_vec();
    bytes.extend(kind.to_le_bytes());
    bytes.extend(payload);
    bytes
}

fn properties() -> Vec<u8> {
    let mut dictionary = name("Required");
    dictionary.extend(name("DefaultValue"));
    // A property group begins with its own name-frame length, not a block tag.
    let field = name("Amount");
    let mut group = ((4 + field.len()) as u32).to_le_bytes().to_vec();
    group.extend(field);
    group.extend([9, 0, 1, 1, 0, 0, 1, 0, 0]);
    // DDL Memo dictionary member1, literal Currency default rendered at scale4.
    group.extend([20, 0, 1, 12, 1, 0, 12, 0]);
    group.extend("0.0000".encode_utf16().flat_map(u16::to_le_bytes));
    let mut bytes = b"MR2\0".to_vec();
    bytes.extend(block(128, &dictionary));
    bytes.extend(block(1, &group));
    bytes
}

/// Empty Table1(ID Increment PrimaryKey, Amount Currency RequiredFalse/default0).
/// Metadata, properties and records are authored without production codecs.
pub fn source() -> Vec<u8> {
    let mut bytes =
        canonical_created_default_zero::source(canonical_created_default_zero::Source::IdOnly);
    let declaration = initial::encode_columns(&COLUMNS);
    bytes[7 * PAGE..8 * PAGE].copy_from_slice(&declaration.bytes[7 * PAGE..8 * PAGE]);
    desktop::definition(&mut bytes[7 * PAGE..8 * PAGE]);

    let page = &mut bytes[4 * PAGE..5 * PAGE];
    assert_eq!(word(page, 12), 8);
    let old_start = word(page, 28);
    let end = word(page, 26);
    let record = canonical_creation_properties::attach(&page[old_start..end], &properties());
    let start = end - record.len();
    page[start..end].copy_from_slice(&record);
    put16(page, 28, start as u16);
    put16(page, 2, (start - 30) as u16);
    bytes
}

/// Append the exact desktop Currency declaration at actual EOF, preserving all
/// old tables, properties, physical slots and retired storage roles.
pub fn append_table(before: &[u8], name: &str) -> Vec<u8> {
    super::append_template(before, name, &source(), Some(&properties()))
}

fn declaration(before: &[u8], definition: u32) {
    let at = definition as usize * PAGE;
    assert_eq!(&before[at + 41..at + 47], &[2, 0, 0, 0, 2, 0]);
    assert_eq!(before[at + 75], 4);
    assert_eq!(before[at + 90], 7);
    assert_eq!(before[at + 100], 5);
    assert_eq!(before[at + 115], 3);
    assert_eq!(&before[at + 121..at + 125], &[4, 0, 8, 0]);
}

/// Literal resolved row values; Omitted uses the authored scale4 zero default.
pub fn row(input: Input, id: i32) -> Vec<Value> {
    vec![
        Value::Int32(id),
        match input {
            Input::Omitted => Value::Currency(Currency::from_scaled(0)),
            Input::Null => Value::Null,
            Input::Value(value) => Value::Currency(Currency::from_scaled(value)),
        },
    ]
}

/// A fresh fifteen-byte native row and its independently tracked ID/ROW key.
pub fn append(before: &[u8], definition: u32, input: Input) -> Vec<u8> {
    declaration(before, definition);
    let pages = Pages {
        definition: definition as usize,
        data: definition as usize + 1,
        index: definition as usize + 2,
    };
    let id = placement::next_id(before, pages);
    let mut record = vec![2, 0];
    record.extend(id.to_le_bytes());
    let value = match input {
        Input::Omitted | Input::Null => 0,
        Input::Value(value) => value,
    };
    record.extend(value.to_le_bytes());
    record.push(if matches!(input, Input::Null) { 1 } else { 3 });
    placement::append(before, pages, id, &record)
}

/// Update only the declared eight-byte Currency range and its presence bit.
/// NULL retains ignored payload bytes; no-op retains the entire source image.
pub fn update(before: &[u8], definition: u32, logical_root: usize, value: Option<i64>) -> Vec<u8> {
    declaration(before, definition);
    let range = raw_forwarding_chain::terminal_range(before, definition + 1, logical_root);
    let record = &before[range.clone()];
    assert_eq!(record.len(), 15);
    assert_eq!(&record[..2], &[2, 0]);
    let original =
        (record[14] & 2 != 0).then(|| i64::from_le_bytes(record[6..14].try_into().unwrap()));
    // NO_CLONE: The independently mutable AFTER must not alter the caller's
    // retained BEFORE oracle, including ignored payload bytes and sibling pages.
    let mut after = before.to_vec();
    if original == value {
        return after;
    }
    let changed = &mut after[range];
    if let Some(value) = value {
        changed[6..14].copy_from_slice(&value.to_le_bytes());
        changed[14] |= 2;
    } else {
        changed[14] &= !2;
    }
    after
}

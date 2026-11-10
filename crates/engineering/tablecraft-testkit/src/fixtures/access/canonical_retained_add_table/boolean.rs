//! Desktop Yes/No has a value bit, no payload bytes and no NULL storage state.
use super::desktop;
use crate::fixtures::access::{
    canonical_initial_table_creation::Column,
    generated_record_insert::placement::{self, Pages},
    raw_forwarding_chain,
};
use tablecraft_core::values::Value;

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
        name: "Enabled",
        native_type: 1,
        fixed_offset: Some(0),
        width: 0,
        variable_index: 0,
        precision: 0,
        scale: 0,
    },
];

#[derive(Clone, Copy, Debug)]
pub enum Input {
    Omitted,
    Value(bool),
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
    let field = name("Enabled");
    let mut group = ((4 + field.len()) as u32).to_le_bytes().to_vec();
    group.extend(field);
    group.extend([9, 0, 1, 1, 0, 0, 1, 0, 0]);
    // Actual current creator authors False but does not author DisplayControl.
    group.extend([18, 0, 1, 12, 1, 0, 10, 0]);
    group.extend("False".encode_utf16().flat_map(u16::to_le_bytes));
    let mut bytes = b"MR2\0".to_vec();
    bytes.extend(block(128, &dictionary));
    bytes.extend(block(1, &group));
    bytes
}

/// Exact current desktop Table1(ID Increment PrimaryKey, Enabled Yes/No).
/// RequiredFalse/defaultFalse are authored; DisplayControl remains absent.
pub fn source() -> Vec<u8> {
    desktop::initial::table(&COLUMNS, &properties())
}

/// Append the same literal declaration at actual EOF beside retained tables.
pub fn append_table(before: &[u8], name: &str) -> Vec<u8> {
    super::append_template(before, name, &source(), Some(&properties()))
}

fn declaration(before: &[u8], definition: u32) {
    let at = definition as usize * PAGE;
    assert_eq!(&before[at + 41..at + 47], &[2, 0, 0, 0, 2, 0]);
    assert_eq!(before[at + 75], 4);
    assert_eq!(before[at + 90], 7);
    assert_eq!(before[at + 100], 1);
    assert_eq!(before[at + 115], 3);
    assert_eq!(&before[at + 121..at + 125], &[0; 4]);
}

fn resolved(input: Input) -> bool {
    match input {
        Input::Omitted => false,
        Input::Value(value) => value,
    }
}

pub fn row(input: Input, id: i32) -> Vec<Value> {
    vec![Value::Int32(id), Value::Boolean(resolved(input))]
}

/// Append a seven-byte native row: count2, generated ID4 and one value mask.
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
    record.push(if resolved(input) { 3 } else { 1 });
    placement::append(before, pages, id, &record)
}

/// Change only Enabled's actual value bit, preserving all payload and other bits.
pub fn update(before: &[u8], definition: u32, logical_root: usize, value: bool) -> Vec<u8> {
    declaration(before, definition);
    let range = raw_forwarding_chain::terminal_range(before, definition + 1, logical_root);
    let record = &before[range.clone()];
    assert_eq!(record.len(), 7);
    assert_eq!(&record[..2], &[2, 0]);
    // NO_CLONE: Independent AFTER ownership preserves the caller's immutable
    // BEFORE witness, including other mask bits and every sibling record/page.
    let mut after = before.to_vec();
    let bit = &mut after[range.start + 6];
    *bit = (*bit & !2) | if value { 2 } else { 0 };
    after
}

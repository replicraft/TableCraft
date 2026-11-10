//! Literal mixed LONG/Currency/Text declaration and independent scalar AFTERs.
use super::{block, name};
use crate::fixtures::access::{
    canonical_initial_table_creation::{self as initial, Column},
    canonical_retained_add_table::desktop,
    generated_record_insert::placement::{self, Pages},
    raw_forwarding_chain, raw_record_update,
};

const PAGE: usize = 4096;
const COLUMNS: [Column; 4] = [
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
        name: "Score",
        native_type: 4,
        fixed_offset: Some(4),
        width: 4,
        variable_index: 0,
        precision: 0,
        scale: 0,
    },
    Column {
        name: "Amount",
        native_type: 5,
        fixed_offset: Some(8),
        width: 8,
        variable_index: 0,
        precision: 0,
        scale: 0,
    },
    Column {
        name: "Label",
        native_type: 10,
        fixed_offset: None,
        width: 510,
        variable_index: 0,
        precision: 0,
        scale: 0,
    },
];

fn group(field: &str, default: Option<&str>) -> Vec<u8> {
    let label = name(field);
    let mut bytes = ((4 + label.len()) as u32).to_le_bytes().to_vec();
    bytes.extend(label);
    bytes.extend([9, 0, 1, 1, 0, 0, 1, 0, 0]); // Required(false), dictionary0.
    if let Some(default) = default {
        let encoded = default
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        bytes.extend(((8 + encoded.len()) as u16).to_le_bytes());
        bytes.extend([1, 12, 2, 0]); // DefaultValue, dictionary2, Memo carrier.
        bytes.extend((encoded.len() as u16).to_le_bytes());
        bytes.extend(encoded);
    } else {
        bytes.extend([9, 0, 1, 1, 1, 0, 1, 0, 255]); // AllowZeroLength(true), dictionary1.
    }
    block(1, &bytes)
}

/// Actual desktop defaults: Score0, Amount0.0000, Label omitted is NULL.
/// No production declaration or property encoder participates in this oracle.
pub fn append_table(before: &[u8], table: &str) -> Vec<u8> {
    let mut template = initial::encode_columns(&COLUMNS).bytes;
    desktop::definition(&mut template[7 * PAGE..8 * PAGE]);
    let mut dictionary = name("Required");
    // Constructive declarations list Boolean policy names before defaults.
    dictionary.extend(name("AllowZeroLength"));
    dictionary.extend(name("DefaultValue"));
    let mut body = b"MR2\0".to_vec();
    body.extend(block(128, &dictionary));
    body.extend(group("Score", Some("0")));
    body.extend(group("Amount", Some("0.0000")));
    body.extend(group("Label", None));
    super::super::append_template(before, table, &template, Some(&body))
}

/// Append a generated row with every companion omitted; only literal stored
/// defaults resolve to values. Label is NULL, rather than an empty string.
pub fn append(before: &[u8], definition: u32) -> Vec<u8> {
    let pages = Pages {
        definition: definition as usize,
        data: definition as usize + 1,
        index: definition as usize + 2,
    };
    let id = placement::next_id(before, pages);
    let mut record = vec![4, 0];
    record.extend(id.to_le_bytes());
    record.extend(0_i32.to_le_bytes());
    record.extend(0_i64.to_le_bytes());
    record.extend([18, 0, 18, 0, 1, 0, 7]);
    placement::append(before, pages, id, &record)
}

pub fn update_number(before: &[u8], definition: u32, root: usize, value: Option<i32>) -> Vec<u8> {
    let field = definition as usize * PAGE + 100;
    raw_record_update::number(before, definition + 1, root, field, 1, value)
}

pub fn update_currency(before: &[u8], definition: u32, root: usize, value: Option<i64>) -> Vec<u8> {
    let range = raw_forwarding_chain::terminal_range(before, definition + 1, root);
    let mut after = before.to_vec();
    let record = &mut after[range];
    assert_eq!(&record[..2], &[4, 0]);
    let mask = record.len() - 1;
    if let Some(value) = value {
        record[10..18].copy_from_slice(&value.to_le_bytes());
        record[mask] |= 4;
    } else {
        record[mask] &= !4;
    }
    after
}

pub fn update_text(before: &[u8], definition: u32, root: usize, value: Option<&str>) -> Vec<u8> {
    let field = definition as usize * PAGE + 150;
    raw_record_update::text(before, definition + 1, root, field, 3, value)
}

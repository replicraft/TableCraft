#![cfg(feature = "ace16-write-preview")]

#[path = "ace16_page_updates/boundaries.rs"]
mod boundaries;
#[allow(dead_code)]
#[path = "ace16_encoded_delete_contract/model.rs"]
mod encoded_model;
use tablecraft_testkit::access::mutation_fixture as fixture;
#[allow(dead_code)]
#[path = "mutation/index_fixture.rs"]
mod index_fixture;
#[path = "ace16_page_updates/model.rs"]
mod model;
use tablecraft_testkit::access as support;
#[path = "ace16_page_updates/values.rs"]
mod values;

use model::{Model, budget, capture, entries, wire};
use std::io::Cursor;
use tablecraft_ace::{EditError, SlotScalarUpdate, check_table, read_table_index};
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;

fn preview(
    model: &Model,
    updates: &[SlotScalarUpdate],
) -> Result<tablecraft_ace::Ace16WritePreview, EditError> {
    capture(&model.bytes).preview_ace16_page_updates(
        "Items",
        "Score",
        updates,
        &mut budget(),
        &|| false,
    )
}

fn word(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap())
}

fn raw_row(bytes: &[u8], slot: usize) -> &[u8] {
    let base = 12 * 4096;
    let start = usize::from(word(bytes, base + 14 + 2 * slot) & 0x1fff);
    let end = if slot == 0 {
        4096
    } else {
        usize::from(word(bytes, base + 12 + 2 * slot) & 0x1fff)
    };
    &bytes[base + start..base + end]
}

fn assert_value(bytes: &[u8], row: usize, value: Value) {
    let table = read_table_index(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        "ScoreKey",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let actual = table
        .rows()
        .iter()
        .find(|values| values[0] == Value::Int32(row as i32 + 1))
        .unwrap();
    assert_eq!(actual[1], value);
    check_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
}

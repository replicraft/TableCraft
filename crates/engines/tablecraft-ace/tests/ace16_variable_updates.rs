#![cfg(feature = "ace16-write-preview")]

#[allow(dead_code)]
#[path = "ace16_encoded_delete_contract/model.rs"]
mod encoded_model;
#[allow(dead_code)]
#[path = "mutation/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "mutation/index_fixture.rs"]
mod index_fixture;
#[allow(dead_code)]
#[path = "ace16_page_updates/model.rs"]
mod page_model;
use tablecraft_testkit::access as support;
#[path = "ace16_variable_updates/values.rs"]
mod values;

use page_model::{Model, budget, capture, entries, wire};
use std::io::Cursor;
use tablecraft_ace::{EditError, SlotScalarUpdate, check_table, read_table_index};
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;

fn source(mut model: Model, alternate: bool) -> Model {
    let mut rows = model
        .records
        .iter()
        .cloned()
        .map(|record| (0, record))
        .collect::<Vec<_>>();
    let payload = model.records.iter().map(Vec::len).sum::<usize>();
    let padding = 4096 - 14 - 2 * (rows.len() + 1) - payload - 10;
    rows.push((0x8000, vec![0x9a; padding]));
    model.bytes[12 * 4096..13 * 4096].copy_from_slice(&support::data_page(10, &rows));
    page_model::fix_free(&mut model.bytes, 12);
    let owned = if alternate {
        vec![0, 0, 0, 0, 0, 0, 0x10, 4]
    } else {
        vec![0, 0, 0, 0, 0, 0, 0x10, 0]
    };
    let hint = owned.clone();
    model.bytes[11 * 4096..12 * 4096]
        .copy_from_slice(&support::data_page(0, &[(0, owned), (0, hint)]));
    if alternate {
        model.bytes[18 * 4096..19 * 4096]
            .copy_from_slice(&support::data_page(10, &[(0x8000, vec![0xb7; 7])]));
        page_model::fix_free(&mut model.bytes, 18);
    }
    model
}

fn preview(
    model: &Model,
    row: usize,
    before: Value,
    after: Value,
) -> Result<tablecraft_ace::Ace16WritePreview, EditError> {
    capture(&model.bytes).preview_ace16_variable_updates(
        "Items",
        "Score",
        &[SlotScalarUpdate::new(row, before, after)],
        &mut budget(),
        &|| false,
    )
}

fn assert_value(bytes: &[u8], row: usize, value: Value) {
    let checked = check_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(
        checked
            .indexes()
            .iter()
            .all(|index| index.status() == tablecraft_ace::IndexCheckStatus::Checked)
    );
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
}

fn slot_body(bytes: &[u8], page: usize, slot: usize) -> &[u8] {
    let base = page * 4096;
    let word = |slot| {
        u16::from_le_bytes(
            bytes[base + 14 + 2 * slot..base + 16 + 2 * slot]
                .try_into()
                .unwrap(),
        ) & 0x1fff
    };
    let start = usize::from(word(slot));
    let end = if slot == 0 {
        4096
    } else {
        usize::from(word(slot - 1))
    };
    &bytes[base + start..base + end]
}

#![cfg(feature = "ace16-write-preview")]

#[allow(dead_code)]
#[path = "ace16_cascading_delete_contract/model.rs"]
mod cascade_model;
#[allow(dead_code)]
#[path = "mutation/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "ace16_restrict_delete_contract/model.rs"]
mod restrict_model;
use tablecraft_testkit::access as support;

#[path = "ace16_cascade_key_facts_contract/tuple.rs"]
mod tuple;

use cascade_model::{before, budget, capture, requests, run, verify};
use std::io::Cursor;
use tablecraft_ace::{TableRowDeletes, read_table_projection};
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;

const PAGE_BYTES: usize = 4096;
const COLUMN_DECLARATIONS: usize = 63;
const INDEX_COUNTER_BYTES: usize = 12;

#[test]
fn association_rows_use_dense_cells_but_primary_metadata_uses_native_positions() {
    let model = cascade_model::database(6, true);
    before(&model);
    let projected = read_table_projection(
        &mut Cursor::new(&model.bytes),
        CatalogScope::new(1),
        "Parents",
        &["Score"],
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(projected.columns().len(), 1);
    assert_eq!(projected.schema().columns()[1].name(), "Score");
    assert_eq!(
        projected.rows(),
        &[
            vec![Value::Int32(10)],
            vec![Value::Int32(20)],
            vec![Value::Int32(30)]
        ]
    );
    let candidate = run(&model, &[(1, &[0])]).unwrap();
    verify(&model, &candidate, [&[0, 1], &[0], &[]]);
}

#[test]
fn reversed_composite_key_order_is_independent_of_dense_and_native_row_order() {
    let model = tuple::database();
    before(&model);
    let projected = read_table_projection(
        &mut Cursor::new(&model.bytes),
        CatalogScope::new(1),
        "Parents",
        &["K2", "Score"],
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        projected
            .columns()
            .iter()
            .map(|column| column.name())
            .collect::<Vec<_>>(),
        ["Score", "K2"]
    );
    assert_eq!(projected.rows()[1], [Value::Int32(20), Value::Int32(200)]);
    let candidate = run(&model, &[(1, &[1])]).unwrap();
    verify(&model, &candidate, [&[1], &[1], &[0]]);
}

#[test]
fn zero_key_scope_keeps_raw_rows_without_activating_unrelated_cascades() {
    let mut model = restrict_model::database(6, false, false, false);
    cascade_model::primary(&mut model, 1);
    restrict_model::relationships(&mut model, 0x1000);
    for (target, logical) in [(0, 0), (1, 1)] {
        let at =
            cascade_model::definition(target) * PAGE_BYTES + model.logical_start + logical * 28;
        model.bytes[at + 22] = 1;
    }
    before(&model);
    let projected = read_table_projection(
        &mut Cursor::new(&model.bytes),
        CatalogScope::new(1),
        "Other",
        &[],
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(projected.columns().is_empty());
    assert_eq!(projected.rows(), &[Vec::<Value>::new()]);
    let selected = requests(&model, 2, &[0]);
    let candidate = capture(&model.bytes)
        .preview_ace16_cascading_deletes(
            &[TableRowDeletes::new("Other", &selected)],
            &mut budget(),
            &|| false,
        )
        .unwrap();
    verify(&model, &candidate, [&[], &[], &[0]]);
}

#[test]
fn projected_facts_do_not_bypass_complete_unselected_field_validation() {
    let mut model = cascade_model::database(6, true);
    before(&model);
    // First unselected Id column becomes unsupported while the selected FK
    // Score remains readable. The complete preflight must still refuse.
    let native = cascade_model::definition(1) * PAGE_BYTES;
    model.bytes[native + COLUMN_DECLARATIONS + INDEX_COUNTER_BYTES] = 0xff;
    let projected = read_table_projection(
        &mut Cursor::new(&model.bytes),
        CatalogScope::new(1),
        "Parents",
        &["Score"],
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(projected.rows()[0], [Value::Int32(10)]);
    assert!(run(&model, &[(1, &[0])]).is_err());
}

//! Independent keyless append images retain native holes and retired defaults.
use super::*;
use crate::{InsertFieldValue, read_table};
use std::io::Cursor;
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    self as raw, field_changes, fields,
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 1_000_000, 500_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture_retained(
        &mut Cursor::new(bytes),
        CatalogScope::new(4610),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn source() -> (Vec<u8>, u32) {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let definition = (seed.len() / 4096) as u32;
    let root = usize::from(u16::from_le_bytes(
        seed[4 * 4096 + 12..4 * 4096 + 14].try_into().unwrap(),
    ));
    let before = field_changes::keyless_first_field(&seed, "Keyless");
    (
        field_changes::drop_field(&before, definition, root, "Id"),
        definition,
    )
}

fn assert_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len(), "whole image length");
    assert_eq!(
        actual.iter().zip(expected).position(|(a, b)| a != b),
        None,
        "first different byte"
    );
}

#[test]
fn keyless_original_presence_appends_literal_nulls_and_value_without_resurrecting_id() {
    let (before, definition) = source();
    let native = image(&before);
    assert!(matches!(
        crate::table::mutation::fixed_records::indexed::CheckedIndexes::capture_actions(
            &native,
            "Keyless",
            &mut budget(),
            &|| false,
        ),
        Err(EditError::Unsupported(WriteBlocker::IndexedColumn))
    ));
    assert!(
        native
            .select_native_retained_keyless_insert_table("Keyless", &mut budget(), &|| false)
            .unwrap()
    );
    native
        .check_native_retained_keyless_insert_table("Keyless", &mut budget(), &|| false)
        .unwrap();
    let empty = native
        .plan_retained_keyless_omitted_row_inserts("Keyless", &[], &mut budget(), &|| false)
        .unwrap();
    assert_image(empty.after_bytes(), &before);
    let requests = [
        OmittedRowInsert::new(vec![InsertFieldValue::Omitted]),
        OmittedRowInsert::new(vec![InsertFieldValue::Supplied(Value::Null)]),
        OmittedRowInsert::new(vec![InsertFieldValue::Supplied(Value::Int32(77))]),
    ];
    let mut expected = before.clone();
    for value in [None, None, Some(77)] {
        expected = field_changes::inserting::keyless_score(&expected, definition, value);
    }
    let plan = image(&before)
        .plan_retained_keyless_omitted_row_inserts("Keyless", &requests, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(plan.changed_rows(), 3);
    assert_image(plan.after_bytes(), &expected);
    let base = definition as usize * 4096;
    assert_eq!(
        &expected[base + 20..base + 24],
        &before[base + 20..base + 24],
        "LAST is not a keyless generator"
    );
    let table = read_table(
        &mut Cursor::new(plan.after_bytes()),
        CatalogScope::new(4611),
        "Keyless",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        table.rows(),
        [
            vec![Value::Int32(23)],
            vec![Value::Null],
            vec![Value::Null],
            vec![Value::Int32(77)]
        ]
    );
    assert_eq!(table.schema().columns()[0].number(), 1);
    image(&expected)
        .check_native_retained_keyless_insert_table("Keyless", &mut budget(), &|| false)
        .unwrap();
    assert!(matches!(
        image(&before).plan_retained_omitted_row_inserts("Keyless", &[], &mut budget(), &|| false),
        Err(EditError::Unsupported(WriteBlocker::Column))
    ));
}

#[test]
fn keyless_empty_requests_still_reject_generated_and_indexed_sources() {
    let source = fields::source(fields::Case::FirstNumber).bytes;
    assert!(matches!(
        image(&source).plan_retained_keyless_omitted_row_inserts(
            "Table1",
            &[],
            &mut budget(),
            &|| false
        ),
        Err(EditError::Unsupported(WriteBlocker::Column))
    ));
    let manual = raw::append(&source, "Manual", raw::Target::ManualNumber);
    let indexed = image(&manual);
    assert!(matches!(
        crate::table::mutation::fixed_records::indexed::CheckedIndexes::capture_keyless_actions(
            &indexed,
            "Manual",
            &mut budget(),
            &|| false,
        ),
        Err(EditError::Unsupported(WriteBlocker::IndexedColumn))
    ));
    assert!(
        !image(&manual)
            .select_native_retained_keyless_insert_table("Manual", &mut budget(), &|| false)
            .unwrap()
    );
    assert!(matches!(
        image(&manual).plan_retained_keyless_omitted_row_inserts(
            "Manual",
            &[],
            &mut budget(),
            &|| false
        ),
        Err(EditError::Unsupported(WriteBlocker::IndexedColumn))
    ));
}

#[test]
fn keyless_full_source_preserves_cancellation_and_budget_errors_for_empty_batches() {
    let (before, _) = source();
    assert!(matches!(
        image(&before).plan_retained_keyless_omitted_row_inserts(
            "Keyless",
            &[],
            &mut budget(),
            &|| true
        ),
        Err(EditError::Inspect(InspectError::Cancelled))
    ));
    let error = image(&before)
        .plan_retained_keyless_omitted_row_inserts(
            "Keyless",
            &[],
            &mut ResourceBudget::new(0, 1_000_000, 500_000_000),
            &|| false,
        )
        .err()
        .unwrap();
    assert!(matches!(error, EditError::Inspect(InspectError::Limit(_))));
}

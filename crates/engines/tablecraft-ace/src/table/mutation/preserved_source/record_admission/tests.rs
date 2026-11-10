//! Actual historical sources retain their original bytes across normal INSERT.
use crate::{
    EditError, FixedColumnUpdates, FixedScalarUpdate, InsertFieldValue, NativeImage,
    OmittedRowInsert, WriteBlocker, table::mutation::normal_ordinary,
};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{
    canonical_created_number::mixed::after_off::{self, populated},
    put16,
};

const SCOPE: CatalogScope = CatalogScope::new(999);
const PAGE: usize = 4096;

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 20_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

#[test]
fn omitted_default_and_explicit_null_inserts_match_independent_mixed_width_images() {
    let [_, mut current] = populated::pair("Bonus", &[Some(42), None, Some(-7)]);
    for (input, stored) in [
        (InsertFieldValue::Omitted, Some(0)),
        (InsertFieldValue::Supplied(Value::Null), None),
    ] {
        let expected = populated::append_row(&current, Some(0), stored);
        let request = OmittedRowInsert::new(vec![
            InsertFieldValue::Omitted,
            InsertFieldValue::Omitted,
            input,
        ]);
        let plan = image(&current)
            .plan_ordinary_omitted_row_inserts("Table1", &[request], &mut budget(), &|| false)
            .unwrap();
        assert_eq!(plan.before_bytes(), current);
        assert!(
            plan.after_bytes() == expected,
            "complete independent INSERT image differs"
        );
        assert_eq!(plan.changed_rows(), 1);
        assert_eq!(
            &plan.after_bytes()[9 * PAGE - 33..9 * PAGE],
            &current[9 * PAGE - 33..9 * PAGE]
        );
        current = expected;
    }
}

#[test]
fn historical_admission_uses_native_counts_instead_of_a_two_to_three_special_case() {
    let [populated_before, _] = populated::pair("Bonus", &[Some(42), None, Some(-7)]);
    let empty = after_off::source(after_off::SourceCase::Number);
    let third = after_off::append_number(&empty, "Bonus");
    let mut fourth = after_off::append_number(&third, "Later");
    fourth[8 * PAGE..10 * PAGE].copy_from_slice(&populated_before[8 * PAGE..10 * PAGE]);
    for offset in [16, 20, 67] {
        let at = 7 * PAGE + offset;
        fourth[at..at + 4].copy_from_slice(&populated_before[at..at + 4]);
    }
    let native = image(&fourth);
    let source =
        normal_ordinary::capture_insert(&native, "Table1", &mut budget(), &|| false).unwrap();
    assert_eq!(
        source.table.rows(),
        &[
            vec![Value::Int32(1), Value::Int32(42), Value::Null, Value::Null],
            vec![Value::Int32(2), Value::Null, Value::Null, Value::Null],
            vec![Value::Int32(3), Value::Int32(-7), Value::Null, Value::Null],
        ]
    );
    for (position, record) in source.records.iter().enumerate() {
        let end = 9 * PAGE - 11 * position;
        assert_eq!(record.data, fourth[end - 11..end]);
        assert_eq!(&record.data[..2], &[2, 0]);
    }
    assert_eq!(native.as_bytes(), fourth);
}

#[test]
fn historical_insert_admission_does_not_grant_update_expansion_or_accept_invalid_counts() {
    let [_, before] = populated::pair("Bonus", &[Some(42), None, Some(-7)]);
    let original = image(&before);
    normal_ordinary::capture_insert(&original, "Table1", &mut budget(), &|| false).unwrap();
    assert!(matches!(
        normal_ordinary::capture(&original, "Table1", &mut budget(), &|| false),
        Err(EditError::Unsupported(WriteBlocker::RowLayout))
    ));
    let updates = [FixedScalarUpdate::new(0, Value::Null, Value::Int32(7))];
    let columns = [FixedColumnUpdates::new("Table1", "Bonus", &updates)];
    assert!(matches!(
        image(&before).plan_ordinary_fixed_updates("Table1", &columns, &mut budget(), &|| false),
        Err(EditError::Unsupported(WriteBlocker::RowLayout))
    ));
    assert_eq!(original.as_bytes(), before);

    for count in [0, 4] {
        let mut damaged = before.clone();
        put16(&mut damaged, 9 * PAGE - 11, count);
        let native = image(&damaged);
        assert!(
            normal_ordinary::capture_insert(&native, "Table1", &mut budget(), &|| false).is_err()
        );
        assert_eq!(native.as_bytes(), damaged);
    }
}

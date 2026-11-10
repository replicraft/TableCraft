//! Independent fresh-row images retain all opaque original record witnesses.
use super::*;
use crate::{InsertFieldValue, InspectError, check_table, inspect_table_schema, read_table};
use std::io::Cursor;
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    opaque_rows::{self as raw, CASES, Case},
    rows::Input,
};

const SCOPE: CatalogScope = CatalogScope::new(4309);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 500_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture_retained(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn request(input: Input<'_>) -> OmittedRowInsert {
    let value = match input {
        Input::Omitted => InsertFieldValue::Omitted,
        Input::Null => InsertFieldValue::Supplied(Value::Null),
        Input::Number(number) => InsertFieldValue::Supplied(Value::Int32(number)),
        Input::Text(_) => panic!("this fixture declares a LONG companion"),
    };
    OmittedRowInsert::new(vec![InsertFieldValue::Omitted, value])
}

fn equal_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    assert!(
        actual == expected,
        "first differing byte {:?}",
        actual.iter().zip(expected).position(|(a, b)| a != b)
    );
}

#[test]
fn retained_opaque_insert_preserves_original_tails_and_literal_presence_defaults() {
    for case in CASES {
        let source = raw::source(case);
        // Actual18 changes one row; the other original row remains ambiguous.
        let mut before = raw::after(&source.bytes, source.definition, 0, Some(123));
        let sibling = read_table(
            &mut Cursor::new(&before),
            SCOPE,
            "Other",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let native = image(&before);
        assert!(
            native
                .select_native_retained_opaque_insert_table("Table1", &mut budget(), &|| false)
                .unwrap()
        );
        native
            .check_native_retained_opaque_insert_table("Table1", &mut budget(), &|| false)
            .unwrap();
        assert!(matches!(
            native.check_native_retained_insert_table("Table1", &mut budget(), &|| false),
            Err(EditError::Inspect(InspectError::Unsupported))
        ));
        let empty = image(&before)
            .plan_retained_opaque_omitted_row_inserts("Table1", &[], &mut budget(), &|| false)
            .unwrap();
        equal_image(empty.after_bytes(), &before);
        assert_eq!(empty.changed_rows(), 0);

        let mut expected_rows = vec![
            vec![Value::Int32(1), Value::Int32(123)],
            vec![Value::Int32(2), Value::Null],
        ];
        for (id, input) in [Input::Omitted, Input::Null, Input::Number(-42)]
            .into_iter()
            .enumerate()
        {
            let expected = raw::append(&before, source.definition, input);
            let plan = image(&before)
                .plan_retained_opaque_omitted_row_inserts(
                    "Table1",
                    &[request(input)],
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            equal_image(plan.before_bytes(), &before);
            equal_image(plan.after_bytes(), &expected);
            assert_eq!(plan.changed_rows(), 1);
            for &root in &source.roots {
                assert_eq!(
                    raw::record(&before, source.definition, root),
                    raw::record(plan.after_bytes(), source.definition, root),
                    "original terminal root {root} in {case:?}"
                );
            }
            expected_rows.push(vec![
                Value::Int32(i32::try_from(id).unwrap() + 3),
                match input {
                    Input::Omitted => Value::Int32(0),
                    Input::Null => Value::Null,
                    Input::Number(value) => Value::Int32(value),
                    Input::Text(_) => unreachable!(),
                },
            ]);
            let table = read_table(
                &mut Cursor::new(plan.after_bytes()),
                SCOPE,
                "Table1",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(table.rows(), expected_rows);
            let other = read_table(
                &mut Cursor::new(plan.after_bytes()),
                SCOPE,
                "Other",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(other.rows(), sibling.rows());
            check_table(
                &mut Cursor::new(plan.after_bytes()),
                SCOPE,
                "Table1",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            before = expected;
        }
    }
}

#[test]
fn retained_opaque_insert_requires_positive_ambiguity_and_its_own_purpose() {
    let source = raw::source(Case::VariableRetired);
    let first = raw::after(&source.bytes, source.definition, 0, Some(123));
    let complete = raw::after(&first, source.definition, 1, Some(456));
    let native = image(&complete);
    assert!(
        !native
            .select_native_retained_opaque_insert_table("Table1", &mut budget(), &|| false)
            .unwrap()
    );
    native
        .check_native_retained_insert_table("Table1", &mut budget(), &|| false)
        .unwrap();
    assert!(matches!(
        image(&complete).plan_retained_opaque_omitted_row_inserts(
            "Table1",
            &[],
            &mut budget(),
            &|| false
        ),
        Err(EditError::Unsupported(WriteBlocker::RowLayout))
    ));

    let native = image(&source.bytes);
    let schema = inspect_table_schema(
        &mut native.reader(),
        SCOPE,
        "Table1",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let certificate = crate::table::mutation::no_required_programs::NoRequiredTablePrograms::capture_retained_opaque_rows(
        &native,
        &schema,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(matches!(
        crate::table::mutation::preserved_source::capture_retained_opaque_insert(
            &native,
            "Table1",
            &certificate,
            &mut budget(),
            &|| false
        ),
        Err(EditError::Preservation)
    ));
}

#[test]
fn retained_opaque_insert_preserves_cancellation_and_explicit_resource_refusals() {
    let source = raw::source(Case::FixedRetired);
    let native = image(&source.bytes);
    assert!(matches!(
        native.check_native_retained_opaque_insert_table("Table1", &mut budget(), &|| true),
        Err(EditError::Inspect(InspectError::Cancelled))
    ));
    for mut allowance in [
        ResourceBudget::new(0, 1_000_000, 500_000_000),
        ResourceBudget::new(1_000_000_000, 0, 500_000_000),
        ResourceBudget::new(1_000_000_000, 1_000_000, 0),
    ] {
        assert!(matches!(
            image(&source.bytes).plan_retained_opaque_omitted_row_inserts(
                "Table1",
                &[request(Input::Omitted)],
                &mut allowance,
                &|| false
            ),
            Err(EditError::Inspect(InspectError::Limit(_)))
        ));
    }
    assert_eq!(native.as_bytes(), source.bytes);
}

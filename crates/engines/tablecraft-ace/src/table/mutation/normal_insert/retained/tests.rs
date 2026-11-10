//! Retired catalog storage is admitted only by its separately bound row family.
use super::*;
use crate::table::{mutation::normal_ordinary, table_append::Inventory};
use crate::{InsertFieldValue, InspectError, inspect_table_schema, read_table};
use std::io::Cursor;
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{
    canonical_created_number::mixed::after_off::{
        current_insert,
        deleted_field::populated::{CASES, Case, readdition::Target as Field},
    },
    canonical_retained_add_table::{
        self as raw, Target, desktop,
        rows::{self, Input, Profile},
    },
    canonical_table_drop::current::{self as dropped, History},
};

const SCOPE: CatalogScope = CatalogScope::new(3608);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 500_000_000)
}

fn source() -> Vec<u8> {
    let original = dropped::source(Case::TextPresent, Field::ShortText, true, History::Inserted);
    desktop::append(&dropped::after(&original), "Table1", Target::GeneratedText)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture_retained(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

#[test]
fn retained_generated_source_qualifies_tombstones_without_widening_legacy_insert() {
    let before = source();
    let native = image(&before);
    Inventory::capture(&native, &mut budget(), &|| false).unwrap();
    assert!(
        native
            .select_native_retained_insert_source("Table1", &mut budget(), &|| false)
            .unwrap()
    );
    assert!(
        !native
            .select_native_current_insert_source("Table1", &mut budget(), &|| false)
            .unwrap()
    );

    // Standard capture reaches the actual legacy catalog census, rather than
    // refusing early because a differently scoped image was supplied.
    let legacy =
        NativeImage::capture(&mut Cursor::new(&before), SCOPE, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        normal_ordinary::capture_insert(&legacy, "Table1", &mut budget(), &|| false),
        Err(EditError::Unsupported(WriteBlocker::ActiveContent))
    ));
    native
        .check_native_retained_insert_table("Table1", &mut budget(), &|| false)
        .unwrap();
    let plan = native
        .plan_retained_omitted_row_inserts("Table1", &[], &mut budget(), &|| false)
        .unwrap();
    assert_eq!(plan.changed_rows(), 0);
    assert_eq!(plan.before_bytes(), before);
    assert_eq!(plan.after_bytes(), before);
}

#[test]
fn retained_generated_source_preserves_cancellation_and_explicit_work_refusal() {
    let before = source();
    let native = image(&before);
    assert!(matches!(
        native.check_native_retained_insert_table("Table1", &mut budget(), &|| true),
        Err(EditError::Inspect(InspectError::Cancelled))
    ));
    for limited in [
        ResourceBudget::new(0, 1_000_000, 500_000_000),
        ResourceBudget::new(1_000_000_000, 0, 500_000_000),
        ResourceBudget::new(1_000_000_000, 1_000_000, 0),
    ] {
        let mut limited = limited;
        assert!(matches!(
            native.check_native_retained_insert_table("Table1", &mut limited, &|| false),
            Err(EditError::Inspect(InspectError::Limit(_)))
        ));
    }
    assert_eq!(native.as_bytes(), before);
}

fn request(input: Input<'_>) -> OmittedRowInsert {
    let value = match input {
        Input::Omitted => InsertFieldValue::Omitted,
        Input::Null => InsertFieldValue::Supplied(Value::Null),
        Input::Number(number) => InsertFieldValue::Supplied(Value::Int32(number)),
        Input::Text(text) => InsertFieldValue::Supplied(Value::Text(text.into())),
    };
    OmittedRowInsert::new(vec![InsertFieldValue::Omitted, value])
}

fn equal_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    assert!(
        actual == expected,
        "first differing byte: {:?}",
        actual
            .iter()
            .zip(expected)
            .position(|(left, right)| left != right)
    );
}

#[test]
fn retained_generated_inserts_match_independent_rows_across_live_and_retired_tables() {
    for case in CASES {
        for retired in [false, true] {
            for profile in [
                Profile::NativeNumber,
                Profile::NativeText,
                Profile::DesktopNumber,
                Profile::DesktopText,
            ] {
                let original = dropped::source(case, Field::ShortText, true, History::Inserted);
                let source = if retired {
                    dropped::after(&original)
                } else {
                    original.bytes
                };
                let definition = u32::try_from(source.len() / 4096).unwrap();
                let target = match profile {
                    Profile::NativeNumber | Profile::DesktopNumber => Target::GeneratedNumber,
                    Profile::NativeText | Profile::DesktopText => Target::GeneratedText,
                };
                let mut before = match profile {
                    Profile::DesktopNumber | Profile::DesktopText => {
                        desktop::append(&source, "Added", target)
                    }
                    _ => raw::append(&source, "Added", target),
                };
                let number = [Input::Omitted, Input::Null, Input::Number(123)];
                let text = [
                    Input::Omitted,
                    Input::Null,
                    Input::Text("new label"),
                    Input::Text("日本語🦀"),
                ];
                let inputs: &[Input<'_>] = match profile {
                    Profile::NativeNumber | Profile::DesktopNumber => &number,
                    _ => &text,
                };
                let mut expected_rows = Vec::new();
                for (index, &input) in inputs.iter().enumerate() {
                    let expected = rows::append(&before, definition, profile, input);
                    let plan = image(&before)
                        .plan_retained_omitted_row_inserts(
                            "Added",
                            &[request(input)],
                            &mut budget(),
                            &|| false,
                        )
                        .unwrap_or_else(|error| {
                            panic!("{case:?}/{retired}/{profile:?}/{input:?}: {error:?}")
                        });
                    equal_image(plan.after_bytes(), &expected);
                    assert_eq!(plan.changed_rows(), 1);
                    expected_rows.push(rows::row(profile, input, index as i32 + 1));
                    let observed = read_table(
                        &mut Cursor::new(plan.after_bytes()),
                        SCOPE,
                        "Added",
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                    assert_eq!(observed.rows(), expected_rows);
                    before = expected;
                }
                if matches!(profile, Profile::DesktopText) {
                    let input = Input::Text("");
                    let expected = rows::append(&before, definition, profile, input);
                    let plan = image(&before)
                        .plan_retained_omitted_row_inserts(
                            "Added",
                            &[request(input)],
                            &mut budget(),
                            &|| false,
                        )
                        .unwrap();
                    equal_image(plan.after_bytes(), &expected);
                }
            }
        }
    }
}

#[test]
fn retained_generated_insert_returns_to_the_original_sparse_table_after_appending_a_sibling() {
    for case in CASES {
        let mut original = current_insert::source(case, Field::ShortText, true, true);
        let mut before = raw::append(&original, "More", Target::GeneratedNumber);
        for input in [current_insert::Input::Unicode, current_insert::Input::Null] {
            let original_after = current_insert::append(&original, case, Field::ShortText, input);
            let expected = raw::append(&original_after, "More", Target::GeneratedNumber);
            let native = image(&before);
            let schema = inspect_table_schema(
                &mut native.reader(),
                SCOPE,
                "Table1",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let fields = schema
                .columns()
                .iter()
                .map(|field| {
                    if field.name() != current_insert::name(Field::ShortText, true) {
                        return InsertFieldValue::Omitted;
                    }
                    let value = match input {
                        current_insert::Input::Unicode => Value::Text("日本語🦀".into()),
                        current_insert::Input::Null => Value::Null,
                        _ => unreachable!("literal test inputs"),
                    };
                    InsertFieldValue::Supplied(value)
                })
                .collect();
            let plan = native
                .plan_retained_omitted_row_inserts(
                    "Table1",
                    &[OmittedRowInsert::new(fields)],
                    &mut budget(),
                    &|| false,
                )
                .unwrap_or_else(|error| panic!("{case:?}/{input:?}: {error:?}"));
            equal_image(plan.after_bytes(), &expected);
            let sibling = read_table(
                &mut Cursor::new(plan.after_bytes()),
                SCOPE,
                "More",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert!(sibling.rows().is_empty());
            before = expected;
            original = original_after;
        }
    }
}

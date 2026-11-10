//! Independent current INSERT rows distinguish native IDs from physical slots.
use super::*;
use crate::{InsertFieldValue, read_table};
use std::io::Cursor;
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_created_number::mixed::after_off::{
    current_insert::{self as raw, Input},
    deleted_field::populated::{CASES, Case, readdition::Target},
};

const SCOPE: CatalogScope = CatalogScope::new(2707);
const PAGE: usize = 4096;
const DEFINITION: usize = 7 * PAGE;
const LAST: usize = 20;

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 30_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn fields(bytes: &[u8], target: Target, input: Input, renamed: bool) -> Vec<InsertFieldValue> {
    let native = image(bytes);
    let schema = inspect_table_schema(
        &mut native.reader(),
        SCOPE,
        "Table1",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let mut fields = Vec::new();
    for field in schema.columns() {
        let value = if field.name() != raw::name(target, renamed) {
            InsertFieldValue::Omitted
        } else {
            match input {
                Input::Omitted => InsertFieldValue::Omitted,
                Input::Null => InsertFieldValue::Supplied(Value::Null),
                Input::Number123 => InsertFieldValue::Supplied(Value::Int32(123)),
                Input::Empty => InsertFieldValue::Supplied(Value::Text(String::new())),
                Input::Compressed => InsertFieldValue::Supplied(Value::Text("new label".into())),
                Input::Unicode => InsertFieldValue::Supplied(Value::Text("日本語🦀".into())),
            }
        };
        fields.push(value);
    }
    fields
}

fn requests(bytes: &[u8], target: Target, input: Input, renamed: bool) -> Vec<OmittedRowInsert> {
    vec![OmittedRowInsert::new(fields(bytes, target, input, renamed))]
}

fn equal_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    assert!(
        actual == expected,
        "first difference {:?}",
        actual
            .iter()
            .zip(expected)
            .position(|(left, right)| left != right)
    );
}

#[test]
fn current_insert_matches_all_raw_history_images_while_fixed_long_keeps_its_old_route() {
    for case in CASES {
        for target in [Target::Number, Target::ShortText] {
            for filled in [false, true] {
                for renamed in [false, true] {
                    let mut before = raw::source(case, target, filled, renamed);
                    let inputs: &[Input] = match target {
                        Target::Number => &raw::NUMBER_INPUTS,
                        Target::ShortText => &raw::TEXT_INPUTS,
                    };
                    for &input in inputs {
                        let originals = requests(&before, target, input, renamed);
                        let expected = raw::append(&before, case, target, input);
                        let next = i32::from_le_bytes(
                            before[DEFINITION + LAST..DEFINITION + LAST + 4]
                                .try_into()
                                .unwrap(),
                        ) + 1;
                        let current = image(&before)
                            .select_native_current_insert_source("Table1", &mut budget(), &|| false)
                            .unwrap();
                        let plan = if current {
                            image(&before)
                                .check_native_current_insert_table("Table1", &mut budget(), &|| {
                                    false
                                })
                                .unwrap_or_else(|error| panic!(
                                    "{case:?}/{target:?}/{filled}/{renamed}/{input:?} availability: {error:?}"
                                ));
                            image(&before).plan_current_omitted_row_inserts(
                                "Table1",
                                &originals,
                                &mut budget(),
                                &|| false,
                            )
                        } else {
                            assert!(matches!(
                                image(&before).plan_current_omitted_row_inserts(
                                    "Table1",
                                    &originals,
                                    &mut budget(),
                                    &|| false,
                                ),
                                Err(EditError::Unsupported(WriteBlocker::RowLayout))
                            ));
                            image(&before).plan_ordinary_omitted_row_inserts(
                                "Table1",
                                &originals,
                                &mut budget(),
                                &|| false,
                            )
                        }
                        .unwrap_or_else(|error| {
                            panic!("{case:?}/{target:?}/{filled}/{renamed}/{input:?}: {error:?}")
                        });
                        equal_image(plan.after_bytes(), &expected);
                        assert_eq!(plan.changed_rows(), 1);
                        let observed = read_table(
                            &mut Cursor::new(plan.after_bytes()),
                            SCOPE,
                            "Table1",
                            &mut budget(),
                            &|| false,
                        )
                        .unwrap();
                        assert_eq!(
                            observed.rows().last(),
                            Some(&raw::row(case, target, input, next))
                        );
                        before = expected;
                    }
                }
            }
        }
    }
}

#[test]
fn current_insert_retains_opaque_binary_siblings_and_accepts_explicit_binary() {
    let case = Case::MixedBinary;
    for target in [Target::Number, Target::ShortText] {
        let before = raw::source(case, target, true, true);
        let input = match target {
            Target::Number => Input::Number123,
            Target::ShortText => Input::Unicode,
        };
        let mut fields = fields(&before, target, input, true);
        fields[3] = InsertFieldValue::Supplied(Value::Binary(vec![0, 255, 1, 0]));
        let originals = [OmittedRowInsert::new(fields)];
        let expected = raw::append_with_binary(&before, case, target, input, &[0, 255, 1, 0]);
        let plan = image(&before)
            .plan_current_omitted_row_inserts("Table1", &originals, &mut budget(), &|| false)
            .unwrap();
        equal_image(plan.after_bytes(), &expected);
    }
}

#[test]
fn current_insert_preserves_omission_guards_empty_operations_and_terminal_controls() {
    let before = raw::source(Case::TextPresent, Target::ShortText, true, true);
    let mut fields = fields(&before, Target::ShortText, Input::Omitted, true);
    fields[0] = InsertFieldValue::Supplied(Value::Null);
    let invalid = [OmittedRowInsert::new(fields)];
    assert!(matches!(
        image(&before)
            .plan_current_omitted_row_inserts("Table1", &invalid, &mut budget(), &|| false,),
        Err(EditError::Unsupported(WriteBlocker::Column))
    ));
    assert!(matches!(
        image(&before).plan_current_omitted_row_inserts("Table1", &[], &mut budget(), &|| true,),
        Err(EditError::Inspect(InspectError::Cancelled))
    ));
    assert!(matches!(
        image(&before).plan_current_omitted_row_inserts(
            "Table1",
            &[],
            &mut ResourceBudget::new(0, 0, 0),
            &|| false,
        ),
        Err(EditError::Inspect(InspectError::Limit(_)))
    ));
    let empty = image(&before)
        .plan_current_omitted_row_inserts("Table1", &[], &mut budget(), &|| false)
        .unwrap();
    equal_image(empty.after_bytes(), &before);
    assert_eq!(empty.changed_rows(), 0);
    let originals = requests(&before, Target::ShortText, Input::Null, true);
    assert!(
        image(&before)
            .plan_ordinary_omitted_row_inserts("Table1", &originals, &mut budget(), &|| false,)
            .is_err()
    );
}

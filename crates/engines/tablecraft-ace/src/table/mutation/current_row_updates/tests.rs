//! Readded fields materialize through actual sparse identities and retained bytes.
use super::*;
use crate::{FixedScalarUpdate, SlotScalarUpdate, read_table};
use std::io::Cursor;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_created_number::mixed::after_off::deleted_field::populated::{
    CASES, readdition::{self as raw, Target},
};

const SCOPE: CatalogScope = CatalogScope::new(1616);

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 30_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn assert_image(actual: &[u8], expected: &[u8]) {
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
fn current_rows_match_all_independent_readdition_materialization_images() {
    for case in CASES {
        for target in [Target::Number, Target::ShortText] {
            let mut before = raw::after(&raw::source(case), target);
            let mut expected_rows = raw::expected(case);
            image(&before)
                .check_native_current_rows_table("Table1", &mut budget(), &|| false)
                .unwrap_or_else(|error| panic!("{case:?}/{target:?} availability: {error:?}"));
            for root in 0..expected_rows.len() {
                let expected = match target {
                    Target::Number => raw::fill_number(case, &before, root, 123),
                    Target::ShortText => raw::fill_text(case, &before, root, "fresh value"),
                };
                let fixed = [FixedScalarUpdate::new(root, Value::Null, Value::Int32(123))];
                let variable = [SlotScalarUpdate::new(
                    root,
                    Value::Null,
                    Value::Text("fresh value".into()),
                )];
                let groups = [match target {
                    Target::Number => FittedColumnUpdates::Fixed {
                        column: target.name(),
                        updates: &fixed,
                    },
                    Target::ShortText => FittedColumnUpdates::Variable {
                        column: target.name(),
                        updates: &variable,
                    },
                }];
                assert!(
                    image(&before)
                        .current_row_updates_selected("Table1", &groups, &mut budget(), &|| false,)
                        .unwrap()
                );
                let plan = image(&before)
                    .plan_current_row_updates("Table1", &groups, &mut budget(), &|| false)
                    .unwrap_or_else(|error| panic!("{case:?}/{target:?}/root{root}: {error:?}"));
                assert_eq!(plan.changed_rows(), 1);
                assert_image(plan.after_bytes(), &expected);
                *expected_rows[root].last_mut().unwrap() = match target {
                    Target::Number => Value::Int32(123),
                    Target::ShortText => Value::Text("fresh value".into()),
                };
                let observed = read_table(
                    &mut Cursor::new(plan.after_bytes()),
                    SCOPE,
                    "Table1",
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(observed.rows(), expected_rows);
                before = expected;
            }
        }
    }
}

#[test]
fn current_text_repeated_growth_empty_and_null_preserve_other_historical_fields() {
    for case in CASES {
        let target = Target::ShortText;
        let mut before = raw::after(&raw::source(case), target);
        let mut previous = Value::Null;
        for (position, text) in ["first", "雪🦀", "a substantially longer replacement", ""]
            .into_iter()
            .enumerate()
        {
            let expected = if position == 0 {
                raw::fill_text(case, &before, 0, text)
            } else {
                raw::replace_text(&before, 0, Some(text))
            };
            let value = Value::Text(text.into());
            // NO_CLONE: the request owns its replacement payload while the loop
            // retains an independent expected value for the next consuming edit.
            let updates = [SlotScalarUpdate::new(0, previous, value.clone())];
            let groups = [FittedColumnUpdates::Variable {
                column: target.name(),
                updates: &updates,
            }];
            let plan = image(&before)
                .plan_current_row_updates("Table1", &groups, &mut budget(), &|| false)
                .unwrap();
            assert_image(plan.after_bytes(), &expected);
            previous = value;
            before = expected;
        }
        let expected = raw::replace_text(&before, 0, None);
        let updates = [SlotScalarUpdate::new(0, previous, Value::Null)];
        let groups = [FittedColumnUpdates::Variable {
            column: target.name(),
            updates: &updates,
        }];
        let plan = image(&before)
            .plan_current_row_updates("Table1", &groups, &mut budget(), &|| false)
            .unwrap();
        assert_image(plan.after_bytes(), &expected);
        let observed = read_table(
            &mut Cursor::new(plan.after_bytes()),
            SCOPE,
            "Table1",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(observed.rows(), raw::expected(case));
    }
}

#[test]
fn current_requests_keep_legacy_sources_closed_and_preserve_expected_value_guards() {
    let before = raw::after(&raw::source(CASES[0]), Target::ShortText);
    let updates = [SlotScalarUpdate::new(
        0,
        Value::Null,
        Value::Text("fresh value".into()),
    )];
    let groups = [FittedColumnUpdates::Variable {
        column: Target::ShortText.name(),
        updates: &updates,
    }];
    assert!(
        image(&before)
            .plan_historical_short_text_updates("Table1", &groups, &mut budget(), &|| false,)
            .is_err()
    );
    assert!(matches!(
        image(&before).plan_current_row_updates("Table1", &groups, &mut budget(), &|| true,),
        Err(EditError::Inspect(InspectError::Cancelled))
    ));
    let stale = [SlotScalarUpdate::new(
        0,
        Value::Text("retired value".into()),
        Value::Text("fresh".into()),
    )];
    let stale_groups = [FittedColumnUpdates::Variable {
        column: Target::ShortText.name(),
        updates: &stale,
    }];
    assert!(matches!(
        image(&before).plan_current_row_updates("Table1", &stale_groups, &mut budget(), &|| false,),
        Err(EditError::Conflict)
    ));
    let indexed = [FixedScalarUpdate::new(0, Value::Int32(1), Value::Int32(99))];
    let indexed_groups = [FittedColumnUpdates::Fixed {
        column: "ID",
        updates: &indexed,
    }];
    assert!(matches!(
        image(&before)
            .plan_current_row_updates("Table1", &indexed_groups, &mut budget(), &|| false,),
        Err(EditError::Unsupported(WriteBlocker::Column))
    ));
}

mod empty_text;

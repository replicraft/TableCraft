//! Whole-image SOURCE/AFTERs precede application and recovery integration.
use super::*;
use crate::{IndexCheckStatus, SlotScalarUpdate, check_table, read_table};
use std::io::Cursor;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_created_number::mixed::after_off::populated_text as raw;

const SCOPE: CatalogScope = CatalogScope::new(1415);

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 30_000_000)
}
fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

#[test]
fn historical_short_text_updates_matches_independent_images_and_keeps_id_keys() {
    for before in [
        raw::pair("NewLabel", &[Some(42), None, Some(-7)])[1].to_vec(),
        raw::mixed_pair()[1].to_vec(),
        raw::forwarded_pair()[1].to_vec(),
    ] {
        for (slot, value) in [
            (0, ""),
            (0, "ab"),
            (0, "latin"),
            (0, "雪🦀"),
            (1, "nullable"),
            (2, ""),
        ] {
            let expected = raw::after(&before, slot, value);
            for bytes in [&before, &expected] {
                let checked = check_table(
                    &mut Cursor::new(bytes),
                    SCOPE,
                    "Table1",
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(checked.rows(), 3);
                assert!(
                    checked
                        .indexes()
                        .iter()
                        .all(|index| index.status() == IndexCheckStatus::Checked)
                );
            }
            let updates = [SlotScalarUpdate::new(
                slot,
                Value::Null,
                Value::Text(value.into()),
            )];
            let groups = [FittedColumnUpdates::Variable {
                column: "NewLabel",
                updates: &updates,
            }];
            assert!(
                image(&before)
                    .historical_short_text_updates_selected(
                        "Table1",
                        &groups,
                        &mut budget(),
                        &|| false
                    )
                    .unwrap()
            );
            let plan = image(&before)
                .plan_historical_short_text_updates("Table1", &groups, &mut budget(), &|| false)
                .unwrap();
            assert!(
                plan.after_bytes() == expected,
                "row {slot}, value {value:?}, difference {:?}",
                plan.after_bytes()
                    .iter()
                    .zip(&expected)
                    .enumerate()
                    .find(|(_, (actual, expected))| actual != expected)
                    .map(|(offset, (actual, expected))| (offset, *actual, *expected))
            );
            assert_eq!(plan.changed_rows(), 1);
            let rows = read_table(
                &mut Cursor::new(&expected),
                SCOPE,
                "Table1",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            for (position, row) in rows.rows().iter().enumerate() {
                let expected = if position == slot {
                    Value::Text(value.into())
                } else {
                    Value::Null
                };
                assert_eq!(row.last(), Some(&expected));
            }
        }
    }
}

#[test]
fn historical_short_text_null_is_an_exact_noop_and_conflicts_and_controls_still_refuse() {
    let before = raw::pair("NewLabel", &[Some(42), None, Some(-7)])[1].to_vec();
    let updates = [SlotScalarUpdate::new(1, Value::Null, Value::Null)];
    let groups = [FittedColumnUpdates::Variable {
        column: "NewLabel",
        updates: &updates,
    }];
    let plan = image(&before)
        .plan_historical_short_text_updates("Table1", &groups, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(plan.after_bytes(), &before);
    assert_eq!(plan.changed_rows(), 0);
    assert!(matches!(
        image(&before).plan_historical_short_text_updates(
            "Table1",
            &groups,
            &mut budget(),
            &|| true
        ),
        Err(EditError::Inspect(InspectError::Cancelled))
    ));
    for (expected, value) in [
        (Value::Text("wrong".into()), Value::Text("x".into())),
        (Value::Null, Value::Text("🦀".repeat(128))),
        (Value::Null, Value::Int32(7)),
    ] {
        let updates = [SlotScalarUpdate::new(1, expected, value)];
        let groups = [FittedColumnUpdates::Variable {
            column: "NewLabel",
            updates: &updates,
        }];
        assert!(
            image(&before)
                .plan_historical_short_text_updates("Table1", &groups, &mut budget(), &|| false)
                .is_err()
        );
    }
}

#[test]
fn historical_present_text_sequence_preserves_native_count_keys_and_full_independent_images() {
    for declared in [
        raw::pair("NewLabel", &[Some(42), None, Some(-7)])[1].to_vec(),
        raw::mixed_pair()[1].to_vec(),
        raw::forwarded_pair()[1].to_vec(),
        raw::id_only_pair()[1].to_vec(),
    ] {
        image(&declared)
            .check_native_historical_row_table("Table1", &mut budget(), &|| false)
            .unwrap();
        let mut before = raw::after(&declared, 0, "initial");
        let mut expected_value = Value::Text("initial".into());
        let columns = read_table(
            &mut Cursor::new(&before),
            SCOPE,
            "Table1",
            &mut budget(),
            &|| false,
        )
        .unwrap()
        .columns()
        .len();
        for value in [
            Some("substantially larger latin value"),
            Some("é"),
            Some(""),
            None,
            Some("雪🦀"),
            Some("雪🦀"),
        ] {
            let replacement = value.map_or(Value::Null, |value| Value::Text(value.into()));
            let unchanged = expected_value == replacement;
            let expected = if unchanged {
                before.to_vec()
            } else {
                raw::present::after(&before, 0, columns - 1, value)
            };
            let updates = [SlotScalarUpdate::new(0, expected_value, replacement)];
            let groups = [FittedColumnUpdates::Variable {
                column: "NewLabel",
                updates: &updates,
            }];
            assert!(
                image(&before)
                    .historical_short_text_updates_selected(
                        "Table1",
                        &groups,
                        &mut budget(),
                        &|| false
                    )
                    .unwrap()
            );
            let plan = image(&before)
                .plan_historical_short_text_updates("Table1", &groups, &mut budget(), &|| false)
                .unwrap();
            assert!(
                plan.after_bytes() == expected,
                "first difference {:?}",
                plan.after_bytes()
                    .iter()
                    .zip(&expected)
                    .position(|(left, right)| left != right)
            );
            assert_eq!(plan.changed_rows(), usize::from(!unchanged));
            let checked = check_table(
                &mut Cursor::new(&expected),
                SCOPE,
                "Table1",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(checked.rows(), 3);
            assert!(
                checked
                    .indexes()
                    .iter()
                    .all(|index| index.status() == IndexCheckStatus::Checked)
            );
            let rows = read_table(
                &mut Cursor::new(&expected),
                SCOPE,
                "Table1",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(rows.rows()[0].last(), Some(updates[0].value()));
            assert_eq!(rows.rows()[1].last(), Some(&Value::Null));
            assert_eq!(rows.rows()[2].last(), Some(&Value::Null));
            expected_value = value.map_or(Value::Null, |value| Value::Text(value.into()));
            before = expected;
        }
    }
}

#[test]
fn present_sibling_text_keeps_later_fields_absent_and_opaque_null_noops_exact() {
    let before = raw::mixed_pair()[1].to_vec();
    for value in [
        None,
        Some(""),
        Some("雪🦀"),
        Some("substantially larger text"),
    ] {
        let replacement = value.map_or(Value::Null, |text| Value::Text(text.into()));
        let expected = raw::present::after(&before, 1, 1, value);
        let updates = [SlotScalarUpdate::new(1, Value::Null, replacement)];
        let groups = [FittedColumnUpdates::Variable {
            column: "Label",
            updates: &updates,
        }];
        let plan = image(&before)
            .plan_historical_short_text_updates("Table1", &groups, &mut budget(), &|| false)
            .unwrap();
        assert!(plan.after_bytes() == expected);
        assert_eq!(raw::present::record(&expected, 1)[..2], [4, 0]);
        let rows = read_table(
            &mut Cursor::new(&expected),
            SCOPE,
            "Table1",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            &rows.rows()[1][2..],
            &[Value::Null, Value::Text(String::new()), Value::Null]
        );
    }
    let updates = [SlotScalarUpdate::new(
        0,
        Value::Text("stale".into()),
        Value::Null,
    )];
    let groups = [FittedColumnUpdates::Variable {
        column: "Label",
        updates: &updates,
    }];
    assert!(matches!(
        image(&before).plan_historical_short_text_updates(
            "Table1",
            &groups,
            &mut budget(),
            &|| false
        ),
        Err(EditError::Conflict)
    ));
}

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
fn historical_short_text_materialization_matches_independent_images_and_keeps_id_keys() {
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
                0,
                Value::Null,
                Value::Text(value.into()),
            )];
            let groups = [FittedColumnUpdates::Variable {
                column: "NewLabel",
                updates: &updates,
            }];
            assert!(
                image(&before)
                    .historical_short_text_materialization_selected(
                        "Table1",
                        &groups,
                        &mut budget(),
                        &|| false
                    )
                    .unwrap()
            );
            let plan = image(&before)
                .plan_historical_short_text_materialization(
                    "Table1",
                    &groups,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(plan.after_bytes(), &expected);
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
        .plan_historical_short_text_materialization("Table1", &groups, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(plan.after_bytes(), &before);
    assert_eq!(plan.changed_rows(), 0);
    assert!(matches!(
        image(&before).plan_historical_short_text_materialization(
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
                .plan_historical_short_text_materialization(
                    "Table1",
                    &groups,
                    &mut budget(),
                    &|| false
                )
                .is_err()
        );
    }
}

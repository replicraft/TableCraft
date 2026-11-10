//! Independent mixed-row SOURCE/AFTER gate before any variable-prefix writer.
use super::current_schema;
use crate::{IndexCheckStatus, NativeImage, check_table, read_table};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_created_number::mixed::after_off::populated_mixed::{self as raw, Case};

const SCOPE: CatalogScope = CatalogScope::new(1308);
const PAGE: usize = 4096;

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 30_000_000)
}

#[test]
fn mixed_number_add_and_variable_footer_materialization_have_independent_complete_images() {
    for case in [Case::Text, Case::Binary, Case::LaterAbsentText] {
        let [original, before] = raw::pair(case);
        if !matches!(case, Case::LaterAbsentText) {
            let image =
                NativeImage::capture(&mut Cursor::new(&original), SCOPE, &mut budget(), &|| false)
                    .unwrap();
            let added = image
                .prepare_current_number_field_addition("Table1", "Bonus", &mut budget(), &|| false)
                .unwrap();
            assert_eq!(added.after_bytes(), before);
        }
        let expected = raw::after(&before, 1, 37);
        for bytes in [before.as_slice(), expected.as_slice()] {
            let image =
                NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false)
                    .unwrap();
            current_schema::observe(&image, "Table1", &mut budget(), &|| false).unwrap();
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
        let table = read_table(
            &mut Cursor::new(&expected),
            SCOPE,
            "Table1",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        for (position, score) in [Value::Int32(42), Value::Null, Value::Int32(-7)]
            .into_iter()
            .enumerate()
        {
            assert_eq!(
                table.rows()[position][0],
                Value::Int32((position + 1) as i32)
            );
            assert_eq!(table.rows()[position][2], score);
            assert_eq!(
                table.rows()[position][4],
                if position == 1 {
                    Value::Int32(37)
                } else {
                    Value::Null
                }
            );
            if matches!(case, Case::LaterAbsentText) {
                assert_eq!(table.rows()[position][5], Value::Null);
            }
        }
        assert_eq!(table.rows()[0][1], Value::Text("alpha".into()));
        assert_eq!(table.rows()[1][1], Value::Null);
        assert_eq!(table.rows()[2][1], Value::Text("wide".into()));
        if matches!(case, Case::Binary) {
            assert_eq!(table.rows()[0][3], Value::Binary(vec![0, 255, 1, 2]));
            assert_eq!(table.rows()[1][3], Value::Binary(vec![]));
        } else {
            assert_eq!(table.rows()[0][3], Value::Text("☃".into()));
            assert_eq!(table.rows()[1][3], Value::Text(String::new()));
        }
        assert_eq!(table.rows()[2][3], Value::Null);
        assert_eq!(&before[..8 * PAGE], &expected[..8 * PAGE]);
        assert_eq!(&before[9 * PAGE..], &expected[9 * PAGE..]);
    }
}

#[test]
fn mixed_native_writer_matches_raw_payload_preserving_images_and_keeps_fixed_profile_closed() {
    use crate::{FixedColumnUpdates, FixedScalarUpdate};

    for case in [Case::Text, Case::Binary, Case::LaterAbsentText] {
        let [_, before] = raw::pair(case);
        for row in 0..3 {
            let expected = raw::after(&before, row, 37);
            let updates = [FixedScalarUpdate::new(row, Value::Null, Value::Int32(37))];
            let groups = [FixedColumnUpdates::new("Table1", "Bonus", &updates)];
            let image =
                NativeImage::capture(&mut Cursor::new(&before), SCOPE, &mut budget(), &|| false)
                    .unwrap();
            assert!(
                image
                    .historical_mixed_number_materialization_selected(
                        "Table1",
                        &groups,
                        &mut budget(),
                        &|| false
                    )
                    .unwrap()
            );
            let plan = image
                .plan_historical_mixed_number_materialization(
                    "Table1",
                    &groups,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(plan.after_bytes(), expected);
            assert_eq!(plan.changed_rows(), 1);
            let image =
                NativeImage::capture(&mut Cursor::new(&before), SCOPE, &mut budget(), &|| false)
                    .unwrap();
            assert!(
                image
                    .plan_historical_number_materialization(
                        "Table1",
                        &groups,
                        &mut budget(),
                        &|| false
                    )
                    .is_err()
            );
        }
    }
}

#[test]
fn consecutive_mixed_growth_keeps_forwarded_existing_number_updates_available() {
    use crate::{FixedColumnUpdates, FixedScalarUpdate};

    let [_, mut before] = raw::pair(Case::Text);
    for row in 0..3 {
        let expected = raw::after(&before, row, 37 + row as i32);
        let updates = [FixedScalarUpdate::new(
            row,
            Value::Null,
            Value::Int32(37 + row as i32),
        )];
        let groups = [FixedColumnUpdates::new("Table1", "Bonus", &updates)];
        let image =
            NativeImage::capture(&mut Cursor::new(&before), SCOPE, &mut budget(), &|| false)
                .unwrap();
        let plan = image
            .plan_historical_mixed_number_materialization("Table1", &groups, &mut budget(), &|| {
                false
            })
            .unwrap();
        assert_eq!(plan.after_bytes(), expected);
        before = expected;
    }
    let image =
        NativeImage::capture(&mut Cursor::new(&before), SCOPE, &mut budget(), &|| false).unwrap();
    assert!(
        image
            .select_native_historical_present_fixed_table("Table1", &mut budget(), &|| false)
            .unwrap()
    );
    image
        .check_native_historical_present_fixed_table("Table1", &mut budget(), &|| false)
        .unwrap();
    let updates = [FixedScalarUpdate::new(
        0,
        Value::Int32(42),
        Value::Int32(99),
    )];
    let groups = [FixedColumnUpdates::new("Table1", "AfterLabel", &updates)];
    assert!(
        !image
            .historical_mixed_number_materialization_selected(
                "Table1",
                &groups,
                &mut budget(),
                &|| false
            )
            .unwrap()
    );
    let plan = image
        .plan_historical_present_fixed_updates("Table1", &groups, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(plan.after_bytes(), raw::update_number(&before, 0, Some(99)));
    let table = read_table(
        &mut Cursor::new(plan.after_bytes()),
        SCOPE,
        "Table1",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(table.rows()[0][2], Value::Int32(99));
    assert_eq!(table.rows()[0][1], Value::Text("alpha".into()));
    assert_eq!(table.rows()[0][3], Value::Text("☃".into()));
    for row in 0..3 {
        assert_eq!(table.rows()[row][4], Value::Int32(37 + row as i32));
    }
}

#[test]
fn mixed_growth_refuses_bad_offset_tables_noops_wrong_expected_values_and_fixed_sources() {
    use crate::{FixedColumnUpdates, FixedScalarUpdate};
    use tablecraft_testkit::fixtures::access::canonical_created_number::mixed::after_off::populated;

    let [_, before] = raw::pair(Case::Text);
    for (expected, replacement) in [
        (Value::Null, Value::Null),
        (Value::Int32(0), Value::Int32(37)),
        (Value::Null, Value::Boolean(true)),
    ] {
        let updates = [FixedScalarUpdate::new(0, expected, replacement)];
        let groups = [FixedColumnUpdates::new("Table1", "Bonus", &updates)];
        let image =
            NativeImage::capture(&mut Cursor::new(&before), SCOPE, &mut budget(), &|| false)
                .unwrap();
        assert!(
            image
                .plan_historical_mixed_number_materialization(
                    "Table1",
                    &groups,
                    &mut budget(),
                    &|| false
                )
                .is_err()
        );
    }
    let updates = [FixedScalarUpdate::new(0, Value::Null, Value::Int32(37))];
    let groups = [FixedColumnUpdates::new("Table1", "Bonus", &updates)];
    let first = usize::from(u16::from_le_bytes(
        before[8 * PAGE + 14..8 * PAGE + 16].try_into().unwrap(),
    ));
    let mut corrupt = before.to_vec();
    // First row ends at PAGE: offset0 immediately precedes count and mask.
    corrupt[9 * PAGE - 5..9 * PAGE - 3].copy_from_slice(&1_u16.to_le_bytes());
    assert!(first < PAGE - 5);
    if let Ok(image) =
        NativeImage::capture(&mut Cursor::new(&corrupt), SCOPE, &mut budget(), &|| false)
    {
        assert!(
            image
                .plan_historical_mixed_number_materialization(
                    "Table1",
                    &groups,
                    &mut budget(),
                    &|| false
                )
                .is_err()
        );
    }
    let [_, fixed] = populated::pair("Bonus", &[Some(42), None, Some(-7)]);
    let image =
        NativeImage::capture(&mut Cursor::new(&fixed), SCOPE, &mut budget(), &|| false).unwrap();
    assert!(
        !image
            .historical_mixed_number_materialization_selected(
                "Table1",
                &groups,
                &mut budget(),
                &|| false
            )
            .unwrap()
    );
    assert!(
        image
            .plan_historical_mixed_number_materialization("Table1", &groups, &mut budget(), &|| {
                false
            })
            .is_err()
    );
}

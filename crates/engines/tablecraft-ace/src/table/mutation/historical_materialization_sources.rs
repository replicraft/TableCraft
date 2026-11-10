//! Whole-image materialization oracle qualified before adding a writer.
use super::current_schema;
use crate::{IndexCheckStatus, NativeImage, check_table, read_table};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_created_number::mixed::after_off::populated::{
    self, materialization,
};

const PAGE: usize = 4096;
const SCOPE: CatalogScope = CatalogScope::new(1301);

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 30_000_000)
}

#[test]
fn qualify_independent_forwarded_bonus_materialization_without_default_backfill_or_index_rewrite() {
    let [_, ordinary] = populated::pair("Bonus", &[Some(42), None, Some(-7)]);
    let later = materialization::append_number(&ordinary, "Later");
    for mut before in [ordinary, later] {
        let first = usize::from(u16::from_le_bytes(
            before[8 * PAGE + 14..8 * PAGE + 16].try_into().unwrap(),
        ));
        // Bit3 is unused in the historical count2 mask. In the four-field
        // result it must become NULL for Later; bit7 must remain opaque.
        before[8 * PAGE + first + 10] |= 0x88;
        let expected = materialization::after(&before, 0, 2, 37);
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
        assert_eq!(table.rows().len(), 3);
        for (position, score) in [Value::Int32(42), Value::Null, Value::Int32(-7)]
            .into_iter()
            .enumerate()
        {
            assert_eq!(
                table.rows()[position][0],
                Value::Int32((position + 1) as i32)
            );
            assert_eq!(table.rows()[position][1], score);
            assert_eq!(
                table.rows()[position][2],
                if position == 0 {
                    Value::Int32(37)
                } else {
                    Value::Null
                }
            );
            if table.columns().len() == 4 {
                assert_eq!(table.rows()[position][3], Value::Null);
            }
        }
        assert_eq!(&before[..8 * PAGE], &expected[..8 * PAGE]);
        assert_eq!(&before[9 * PAGE..], &expected[9 * PAGE..]);
        let target = usize::from(u16::from_le_bytes(
            expected[8 * PAGE + 20..8 * PAGE + 22].try_into().unwrap(),
        )) & 0x1fff;
        let fields = table.columns().len();
        let mask = expected[8 * PAGE + target + 2 + fields * 4];
        assert_eq!(mask & 0x80, 0x80);
        if fields == 4 {
            assert_eq!(mask & 8, 0);
        }
    }
}

#[test]
fn native_materialization_matches_raw_after_with_other_missing_fields_and_opaque_null_payloads() {
    use crate::{FixedColumnUpdates, FixedScalarUpdate};

    let [_, ordinary] = populated::pair("Bonus", &[Some(42), None, Some(-7)]);
    let later = materialization::append_number(&ordinary, "Later");
    for mut before in [ordinary, later] {
        let second = usize::from(u16::from_le_bytes(
            before[8 * PAGE + 16..8 * PAGE + 18].try_into().unwrap(),
        ));
        // Ignored NULL Score payload survives in the new physical record.
        before[8 * PAGE + second + 6..8 * PAGE + second + 10].fill(0x5a);
        before[8 * PAGE + second + 10] |= 0x88;
        let expected = materialization::after(&before, 1, 2, 37);
        let updates = [FixedScalarUpdate::new(1, Value::Null, Value::Int32(37))];
        let groups = [FixedColumnUpdates::new("Table1", "Bonus", &updates)];
        let image =
            NativeImage::capture(&mut Cursor::new(&before), SCOPE, &mut budget(), &|| false)
                .unwrap();
        assert!(
            image
                .historical_number_materialization_selected(
                    "Table1",
                    &groups,
                    &mut budget(),
                    &|| false
                )
                .unwrap()
        );
        let plan = image
            .plan_historical_number_materialization("Table1", &groups, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(plan.before_bytes(), before);
        assert_eq!(plan.after_bytes(), expected);
        assert_eq!(plan.changed_rows(), 1);
        let after = NativeImage::capture(
            &mut Cursor::new(plan.after_bytes()),
            SCOPE,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        current_schema::observe(&after, "Table1", &mut budget(), &|| false).unwrap();
    }
}

#[test]
fn materialization_refuses_present_targets_null_noops_false_expected_values_and_slot_exhaustion() {
    use crate::{FixedColumnUpdates, FixedScalarUpdate};

    let [_, before] = populated::pair("Bonus", &[Some(42), None, Some(-7)]);
    for (column, expected, replacement) in [
        ("Score", Value::Int32(42), Value::Int32(37)),
        ("Bonus", Value::Null, Value::Null),
        ("Bonus", Value::Int32(0), Value::Int32(37)),
        ("Bonus", Value::Null, Value::Boolean(true)),
    ] {
        let updates = [FixedScalarUpdate::new(0, expected, replacement)];
        let groups = [FixedColumnUpdates::new("Table1", column, &updates)];
        let image =
            NativeImage::capture(&mut Cursor::new(&before), SCOPE, &mut budget(), &|| false)
                .unwrap();
        assert!(
            image
                .plan_historical_number_materialization("Table1", &groups, &mut budget(), &|| false)
                .is_err()
        );
    }
    let updates = [FixedScalarUpdate::new(0, Value::Null, Value::Int32(37))];
    let groups = [FixedColumnUpdates::new("Table1", "Bonus", &updates)];
    let values = vec![Some(1); 255];
    let [_, full] = populated::pair("Bonus", &values);
    let image =
        NativeImage::capture(&mut Cursor::new(&full), SCOPE, &mut budget(), &|| false).unwrap();
    current_schema::observe(&image, "Table1", &mut budget(), &|| false).unwrap();
    assert!(
        image
            .plan_historical_number_materialization("Table1", &groups, &mut budget(), &|| false)
            .is_err()
    );
}

#[test]
fn consecutive_materializations_keep_normal_existing_field_edits_available_after_every_root_is_forwarded()
 {
    use crate::{FixedColumnUpdates, FixedScalarUpdate};

    let [_, mut before] = populated::pair("Bonus", &[Some(42), None, Some(-7)]);
    for row in 0..3 {
        let expected = materialization::after(&before, row, 2, 37 + row as i32);
        let updates = [FixedScalarUpdate::new(
            row,
            Value::Null,
            Value::Int32(37 + row as i32),
        )];
        let groups = [FixedColumnUpdates::new("Table1", "Bonus", &updates)];
        let image =
            NativeImage::capture(&mut Cursor::new(&before), SCOPE, &mut budget(), &|| false)
                .unwrap();
        assert!(
            image
                .select_native_historical_row_table("Table1", &mut budget(), &|| false)
                .unwrap()
        );
        let plan = image
            .plan_historical_number_materialization("Table1", &groups, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(plan.after_bytes(), expected);
        before = expected;
    }
    let expected = populated::update_scores(&before, &[(0, Some(99))]);
    let updates = [FixedScalarUpdate::new(
        0,
        Value::Int32(42),
        Value::Int32(99),
    )];
    let groups = [FixedColumnUpdates::new("Table1", "Score", &updates)];
    let image =
        NativeImage::capture(&mut Cursor::new(&before), SCOPE, &mut budget(), &|| false).unwrap();
    assert!(
        image
            .select_native_historical_row_table("Table1", &mut budget(), &|| false)
            .unwrap()
    );
    image
        .check_native_historical_present_fixed_table("Table1", &mut budget(), &|| false)
        .unwrap();
    assert!(
        !image
            .historical_number_materialization_selected("Table1", &groups, &mut budget(), &|| false)
            .unwrap()
    );
    let plan = image
        .plan_historical_present_fixed_updates("Table1", &groups, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(plan.after_bytes(), expected);
}

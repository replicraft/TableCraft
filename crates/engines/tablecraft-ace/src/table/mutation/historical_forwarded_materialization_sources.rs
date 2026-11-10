//! Independent repeated-growth roots and original index-membership evidence.
use super::current_schema;
use crate::{
    FixedColumnUpdates, FixedScalarUpdate, IndexCheckStatus, NativeImage, check_table, read_table,
};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_created_number::mixed::after_off::{
    populated::materialization::forwarded,
    populated_mixed::{self as mixed, Case},
};

const SCOPE: CatalogScope = CatalogScope::new(1410);

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 60_000_000)
}

fn cases() -> Vec<([Vec<u8>; 2], Vec<u8>)> {
    let fixed = forwarded::pair();
    let fixed_after = forwarded::after_second(&fixed[1], 71);
    let mut result = vec![(fixed, fixed_after)];
    for case in [Case::Text, Case::Binary, Case::LaterAbsentText] {
        let pair = mixed::forwarded_pair(case);
        let after = mixed::forwarded_after(&pair[1], 71);
        result.push((pair, after));
    }
    result
}

fn capture(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

#[test]
fn repeated_growth_source_and_after_retain_original_membership_and_all_old_values() {
    for ([first, before], after) in cases() {
        let addition = capture(&first)
            .prepare_current_number_field_addition("Table1", "SecondBonus", &mut budget(), &|| {
                false
            })
            .unwrap();
        assert_eq!(addition.after_bytes(), before);
        for bytes in [&before, &after] {
            current_schema::observe(&capture(bytes), "Table1", &mut budget(), &|| false).unwrap();
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
        let old = read_table(
            &mut Cursor::new(&before),
            SCOPE,
            "Table1",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let new = read_table(
            &mut Cursor::new(&after),
            SCOPE,
            "Table1",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        for row in 0..3 {
            let fields = old.rows()[row].len();
            assert_eq!(
                &old.rows()[row][..fields - 1],
                &new.rows()[row][..fields - 1]
            );
            assert_eq!(old.rows()[row][fields - 1], Value::Null);
            assert_eq!(
                new.rows()[row][fields - 1],
                if row == 0 {
                    Value::Int32(71)
                } else {
                    Value::Null
                }
            );
        }
        assert_eq!(&before[..8 * 4096], &after[..8 * 4096]);
        assert_eq!(&before[9 * 4096..], &after[9 * 4096..]);
        // Slot3 contains the previous terminal. Growth appends slot4 while the
        // previous slot3 bytes, boundaries and deleted flag remain untouched.
        let data = 8 * 4096;
        let old_word = u16::from_le_bytes(before[data + 20..data + 22].try_into().unwrap());
        let previous = u16::from_le_bytes(before[data + 18..data + 20].try_into().unwrap());
        let start = data + usize::from(old_word & 0x1fff);
        let end = data + usize::from(previous & 0x1fff);
        assert_eq!(&before[start..end], &after[start..end]);
        assert_eq!(&before[data + 20..data + 22], &after[data + 20..data + 22]);
    }
}

#[test]
fn forwarded_materialization_matches_images_without_widening_old_direct_profiles() {
    let updates = [FixedScalarUpdate::new(0, Value::Null, Value::Int32(71))];
    let groups = [FixedColumnUpdates::new("Table1", "SecondBonus", &updates)];
    for ([_, before], expected) in cases() {
        assert!(
            capture(&before)
                .historical_forwarded_number_materialization_selected(
                    "Table1",
                    &groups,
                    &mut budget(),
                    &|| false
                )
                .unwrap()
        );
        assert!(
            capture(&before)
                .plan_historical_number_materialization("Table1", &groups, &mut budget(), &|| false)
                .is_err()
        );
        assert!(
            capture(&before)
                .plan_historical_mixed_number_materialization(
                    "Table1",
                    &groups,
                    &mut budget(),
                    &|| false
                )
                .is_err()
        );
        let plan = capture(&before)
            .plan_historical_forwarded_number_materialization(
                "Table1",
                &groups,
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(plan.after_bytes(), expected);
    }
}

#[test]
fn forwarded_profile_refuses_present_fields_and_does_not_select_direct_missing_rows() {
    let [_, before] = forwarded::pair();
    let updates = [FixedScalarUpdate::new(
        0,
        Value::Int32(37),
        Value::Int32(71),
    )];
    let groups = [FixedColumnUpdates::new("Table1", "Bonus", &updates)];
    assert!(
        !capture(&before)
            .historical_forwarded_number_materialization_selected(
                "Table1",
                &groups,
                &mut budget(),
                &|| false
            )
            .unwrap()
    );
    assert!(
        capture(&before)
            .plan_historical_forwarded_number_materialization(
                "Table1",
                &groups,
                &mut budget(),
                &|| false
            )
            .is_err()
    );
    let updates = [FixedScalarUpdate::new(1, Value::Null, Value::Int32(71))];
    let groups = [FixedColumnUpdates::new("Table1", "SecondBonus", &updates)];
    assert!(
        !capture(&before)
            .historical_forwarded_number_materialization_selected(
                "Table1",
                &groups,
                &mut budget(),
                &|| false
            )
            .unwrap()
    );
    assert!(
        capture(&before)
            .plan_historical_forwarded_number_materialization(
                "Table1",
                &groups,
                &mut budget(),
                &|| false
            )
            .is_err()
    );
}

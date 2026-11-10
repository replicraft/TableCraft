//! Independent whole-image source/expected gate before UPDATE admission.
use super::current_schema;
use crate::{NativeImage, read_table};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_created_number::mixed::after_off::populated;

const PAGE: usize = 4096;
const SCOPE: CatalogScope = CatalogScope::new(1298);

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 30_000_000)
}

#[test]
fn historical_score_update_sources_and_whole_expected_preserve_absent_bonus() {
    let [_, historical] = populated::pair("Bonus", &[Some(42), None, Some(-7)]);
    let mixed = populated::append_row(&historical, Some(11), Some(29));
    for before in [historical.as_slice(), mixed.as_slice()] {
        let expected = populated::update_scores(before, &[(0, Some(8)), (1, Some(12)), (2, None)]);
        for bytes in [before, expected.as_slice()] {
            let image =
                NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false)
                    .unwrap();
            current_schema::observe(&image, "Table1", &mut budget(), &|| false).unwrap();
        }
        let table = read_table(
            &mut Cursor::new(&expected),
            SCOPE,
            "Table1",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            &table.rows()[..3],
            &[
                vec![Value::Int32(1), Value::Int32(8), Value::Null],
                vec![Value::Int32(2), Value::Int32(12), Value::Null],
                vec![Value::Int32(3), Value::Null, Value::Null],
            ]
        );
        if table.rows().len() == 4 {
            assert_eq!(
                table.rows()[3],
                vec![Value::Int32(4), Value::Int32(11), Value::Int32(29)]
            );
        }
        assert_eq!(&before[..8 * PAGE], &expected[..8 * PAGE]);
        assert_eq!(&before[9 * PAGE..], &expected[9 * PAGE..]);
        for (offset, (&old, &new)) in before.iter().zip(&expected).enumerate() {
            if old != new {
                assert!((8 * PAGE..9 * PAGE).contains(&offset));
            }
        }
    }
}

#[test]
fn existing_score_planner_matches_independent_whole_after_and_keeps_new_field_absent() {
    use crate::{FixedColumnUpdates, FixedScalarUpdate};

    let [_, before] = populated::pair("Bonus", &[Some(42), None, Some(-7)]);
    let expected = populated::update_scores(&before, &[(0, Some(8)), (1, Some(12)), (2, None)]);
    let updates = [
        FixedScalarUpdate::new(0, Value::Int32(42), Value::Int32(8)),
        FixedScalarUpdate::new(1, Value::Null, Value::Int32(12)),
        FixedScalarUpdate::new(2, Value::Int32(-7), Value::Null),
    ];
    let groups = [FixedColumnUpdates::new("Table1", "Score", &updates)];
    let image =
        NativeImage::capture(&mut Cursor::new(&before), SCOPE, &mut budget(), &|| false).unwrap();
    let plan = image
        .plan_historical_present_fixed_updates("Table1", &groups, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(plan.before_bytes(), before);
    assert_eq!(plan.after_bytes(), expected);
    assert_eq!(plan.changed_rows(), 3);
}

#[test]
fn historical_missing_target_indexed_target_and_conflicts_still_refuse() {
    use crate::{EditError, FixedColumnUpdates, FixedScalarUpdate, WriteBlocker};

    let [_, before] = populated::pair("Bonus", &[Some(42), None, Some(-7)]);
    for (column, expected, value, blocker) in [
        (
            "Bonus",
            Value::Null,
            Value::Int32(5),
            Some(WriteBlocker::RowLayout),
        ),
        (
            "Bonus",
            Value::Null,
            Value::Null,
            Some(WriteBlocker::RowLayout),
        ),
        ("ID", Value::Int32(1), Value::Int32(9), None),
        ("Score", Value::Int32(99), Value::Int32(8), None),
    ] {
        let updates = [FixedScalarUpdate::new(0, expected, value)];
        let groups = [FixedColumnUpdates::new("Table1", column, &updates)];
        let image =
            NativeImage::capture(&mut Cursor::new(&before), SCOPE, &mut budget(), &|| false)
                .unwrap();
        let error = match image.plan_historical_present_fixed_updates(
            "Table1",
            &groups,
            &mut budget(),
            &|| false,
        ) {
            Ok(_) => panic!("Unqualified historical target was admitted"),
            Err(error) => error,
        };
        if let Some(blocker) = blocker {
            assert_eq!(error, EditError::Unsupported(blocker));
        }
    }
}

#[test]
fn unused_short_record_mask_bits_stay_opaque_and_empty_requests_still_requalify() {
    use crate::{FixedColumnUpdates, FixedScalarUpdate};

    let [_, mut before] = populated::pair("Bonus", &[Some(42), None, Some(-7)]);
    let data = 8 * PAGE;
    let first = usize::from(u16::from_le_bytes(
        before[data + 14..data + 16].try_into().unwrap(),
    ));
    // Count=2: the stored mask's Bonus bit and final unused bit are not values.
    // They must remain untouched rather than becoming a materialized default.
    before[data + first + 10] |= 0x84;
    let expected = populated::update_scores(&before, &[(0, Some(8))]);
    let updates = [FixedScalarUpdate::new(0, Value::Int32(42), Value::Int32(8))];
    let groups = [FixedColumnUpdates::new("Table1", "Score", &updates)];
    let image =
        NativeImage::capture(&mut Cursor::new(&before), SCOPE, &mut budget(), &|| false).unwrap();
    let plan = image
        .plan_historical_present_fixed_updates("Table1", &groups, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(plan.after_bytes(), expected);
    let decoded = read_table(
        &mut Cursor::new(plan.after_bytes()),
        SCOPE,
        "Table1",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(decoded.rows()[0][2], Value::Null);

    let empty = [FixedColumnUpdates::new("Table1", "Score", &[])];
    let image =
        NativeImage::capture(&mut Cursor::new(&before), SCOPE, &mut budget(), &|| false).unwrap();
    let plan = image
        .plan_historical_present_fixed_updates("Table1", &empty, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(plan.after_bytes(), before);
    assert_eq!(plan.changed_rows(), 0);

    let image =
        NativeImage::capture(&mut Cursor::new(&before), SCOPE, &mut budget(), &|| false).unwrap();
    assert!(
        image
            .plan_historical_present_fixed_updates("Table1", &empty, &mut budget(), &|| true)
            .is_err()
    );
}

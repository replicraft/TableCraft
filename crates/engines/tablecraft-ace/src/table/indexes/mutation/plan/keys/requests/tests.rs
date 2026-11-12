//! Literal written order and real numeric key transitions protect shared consumers.
use super::Requests;
use crate::table::indexes::{
    composite::DecimalValue,
    key::TestKeyProfile,
    mutation::{
        IndexKeyUpdate, NumericKey,
        composite::TupleProfile,
        profile::{SingleProfile, WriteProfile},
    },
};
use crate::{EditError, InspectError, inspect_table_indexes};
use std::io::Cursor;
use tablecraft_core::{
    limits::{LimitError, Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;
use tablecraft_testkit::fixtures::access::index_tuples::numeric_fixture;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1024 * 1024, 1000, 100_000)
}

fn profile(column: usize) -> SingleProfile {
    SingleProfile {
        column,
        kind: NumericKey::new(ScalarType::Int32).unwrap(),
        descending: false,
    }
}

fn update(
    reference: u32,
    column: usize,
    expected: Option<i64>,
    value: Option<i64>,
) -> IndexKeyUpdate {
    IndexKeyUpdate {
        reference,
        column,
        expected: expected.map(DecimalValue::Numeric),
        value: value.map(DecimalValue::Numeric),
    }
}

fn updates() -> [IndexKeyUpdate; 7] {
    [
        update(u32::MAX, 1, Some(5), Some(6)),
        update(7, 1, Some(1), Some(2)),
        update(7, 2, Some(8), Some(9)),
        update(2, 1, Some(3), Some(4)),
        update(7, 1, Some(2), None),
        update(7, 2, Some(9), Some(10)),
        update(u32::MAX, 1, Some(6), Some(7)),
    ]
}

#[test]
fn repeated_row_updates_apply_every_written_transition_in_input_order() {
    let updates = updates();
    let profile = profile(1);
    let lookup = Requests::new(&updates, &profile, &mut budget(), &|| false).unwrap();
    assert_eq!(lookup.count(), 5);
    assert!(lookup.has_null_transition());
    // Queries deliberately do not follow ROW order. Equal ranges preserve the
    // caller's transitions, rather than sorting by column or keeping one match.
    for (row, positions, initial, expected) in [
        (u32::MAX, &[0, 6][..], Some(5), Some(7)),
        (7, &[1, 4][..], Some(1), None),
        (2, &[3][..], Some(3), Some(4)),
    ] {
        let selected = lookup.row(row, &mut budget(), &|| false).unwrap();
        assert_eq!(selected, positions);
        let mut key = initial;
        for &position in selected {
            let update = &updates[position];
            profile
                .update(&mut key, update.column, update.expected, update.value)
                .unwrap();
        }
        assert_eq!(key, expected);
    }
    // A late wrong expected value must still be observed after an earlier
    // valid same-ROW transition, not hidden by first-match deduplication.
    let mut key = Some(1);
    profile
        .update(
            &mut key,
            1,
            Some(DecimalValue::Numeric(1)),
            Some(DecimalValue::Numeric(2)),
        )
        .unwrap();
    assert_eq!(
        profile.update(&mut key, 1, Some(DecimalValue::Numeric(99)), None),
        Err(EditError::Conflict)
    );
}

#[test]
fn actual_component_filter_keeps_its_order_and_does_not_count_missing_rows_as_matches() {
    let updates = updates();
    let lookup = Requests::new(&updates, &profile(2), &mut budget(), &|| false).unwrap();
    assert_eq!(lookup.count(), 2);
    assert!(
        !lookup.has_null_transition(),
        "NULL in another component is irrelevant"
    );
    assert_eq!(lookup.row(7, &mut budget(), &|| false).unwrap(), &[2, 5]);
    for missing in [0, 1, 2, 6, 8, u32::MAX] {
        assert!(
            lookup
                .row(missing, &mut budget(), &|| false)
                .unwrap()
                .is_empty()
        );
    }
    // Missing source ROWs do not reduce the expected matched count. The shared
    // prepare owner must still reject after walking a population without ROW7.
    assert_eq!(lookup.count(), 2);
    let component = profile(2);
    let mut key = Some(8);
    for &position in lookup.row(7, &mut budget(), &|| false).unwrap() {
        let update = &updates[position];
        component
            .update(&mut key, update.column, update.expected, update.value)
            .unwrap();
    }
    assert_eq!(key, Some(10));
    let excluded = Requests::new(&updates, &profile(3), &mut budget(), &|| false).unwrap();
    assert_eq!(excluded.count(), 0);
    assert!(!excluded.has_null_transition());
    assert!(
        excluded
            .row(7, &mut budget(), &|| false)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn paid_lookup_cancellation_and_allocation_work_limits_are_typed_refusals() {
    let updates = updates();
    let profile = profile(1);
    assert!(matches!(
        Requests::new(&updates, &profile, &mut budget(), &|| true),
        Err(EditError::Inspect(InspectError::Cancelled))
    ));
    assert!(matches!(
        Requests::new(
            &updates,
            &profile,
            &mut ResourceBudget::new(0, 1000, 100_000),
            &|| false
        ),
        Err(EditError::Inspect(InspectError::Limit(LimitError {
            resource: Resource::Bytes,
            remaining: 0,
        })))
    ));
    assert!(matches!(
        Requests::new(
            &updates,
            &profile,
            &mut ResourceBudget::new(1024, 1000, 0),
            &|| false
        ),
        Err(EditError::Inspect(InspectError::Limit(LimitError {
            resource: Resource::Steps,
            remaining: 0,
        })))
    ));
    let lookup = Requests::new(&updates, &profile, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        lookup.row(7, &mut budget(), &|| true),
        Err(EditError::Inspect(InspectError::Cancelled))
    ));
    assert!(matches!(
        lookup.row(7, &mut ResourceBudget::new(1024, 1000, 0), &|| false),
        Err(EditError::Inspect(InspectError::Limit(LimitError {
            resource: Resource::Steps,
            remaining: 0,
        })))
    ));
}

#[test]
fn real_tuple_profile_keeps_interleaved_components_in_written_row_order() {
    let bytes = numeric_fixture::database(
        5,
        &[4, 4, 4],
        &[(1, false), (2, false)],
        &[vec![Some(0), Some(1), Some(8)]],
        0,
    );
    let inspection = inspect_table_indexes(
        &mut Cursor::new(bytes),
        CatalogScope::new(32630),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let profile = TupleProfile::<i64>::new(
        &inspection.indexes()[0],
        inspection.schema(),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let updates = updates();
    let lookup = Requests::new(&updates, &profile, &mut budget(), &|| false).unwrap();
    assert_eq!(lookup.count(), 7);
    let selected = lookup.row(7, &mut budget(), &|| false).unwrap();
    assert_eq!(selected, &[1, 2, 4, 5]);
    let mut key = profile
        .row_key_test(&[Value::Int32(0), Value::Int32(1), Value::Int32(8)])
        .unwrap();
    let expected = [
        (Some(2), Some(8)),
        (Some(2), Some(9)),
        (None, Some(9)),
        (None, Some(10)),
    ];
    for (&position, expected) in selected.iter().zip(expected) {
        let update = &updates[position];
        profile
            .update(&mut key, update.column, update.expected, update.value)
            .unwrap();
        assert_eq!((key.get(0), key.get(1)), expected);
    }
}

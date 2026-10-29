//! Independent timestamp records qualify before any INSERT producer is invoked.
use super::model as original;
use crate::{RowInsert, read_table};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

mod model;

fn additions() -> Vec<model::Row> {
    vec![
        model::Row {
            id: 10,
            group: Some(10),
            timestamp: Some(model::Timestamp::Leap),
        },
        model::Row {
            id: 11,
            group: Some(11),
            timestamp: Some(model::Timestamp::Tick),
        },
        model::Row {
            id: 12,
            group: None,
            timestamp: None,
        },
        model::Row {
            id: 13,
            group: Some(11),
            timestamp: Some(model::Timestamp::Maximum),
        },
    ]
}

fn checked(bytes: &[u8], rows: &[model::Row]) {
    original::checked(bytes, rows.len(), 4);
    let snapshot = read_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut original::budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        snapshot.rows(),
        rows.iter().map(model::Row::values).collect::<Vec<_>>()
    );
}

fn same_image(actual: &[u8], expected: &[u8]) {
    let difference = actual
        .iter()
        .zip(expected)
        .position(|(actual, expected)| actual != expected);
    assert_eq!(actual.len(), expected.len());
    assert!(
        difference.is_none(),
        "first extended timestamp image difference: {difference:?}"
    );
}

#[test]
fn extended_datetime_source_and_expected_images_qualify_without_insert_execution() {
    let case = model::source(6);
    checked(&case.before, &case.rows);
    let added = additions();
    let expected = model::expected(&case, &added);
    let all = case.rows.iter().cloned().chain(added).collect::<Vec<_>>();
    checked(&expected, &all);
    // Source NULL fixed cells are opaque, while newly inserted NULL payloads
    // have canonical zeroes. Both remain logically NULL to the existing reader.
    let source_start = u16::from_le_bytes(
        case.before[12 * model::PAGE + 16..12 * model::PAGE + 18]
            .try_into()
            .unwrap(),
    ) as usize;
    assert_eq!(
        &case.before[12 * model::PAGE + source_start + 10..12 * model::PAGE + source_start + 52],
        &[0xa5; 42]
    );
}

#[test]
fn extended_datetime_epoch_leap_maximum_and_100ns_insert_match_whole_image() {
    let case = model::source(6);
    let added = additions();
    let expected = model::expected(&case, &added);
    let all = case
        .rows
        .iter()
        .cloned()
        .chain(added.iter().cloned())
        .collect::<Vec<_>>();
    checked(&case.before, &case.rows);
    checked(&expected, &all);
    let requests = added
        .iter()
        .map(|row| RowInsert::new(row.values()))
        .collect::<Vec<_>>();
    let candidate = original::capture(&case.before)
        .preview_ace16_insert_rows("Items", &requests, &mut original::budget(), &|| false)
        .unwrap();
    same_image(candidate.candidate_bytes(), &expected);
    checked(candidate.candidate_bytes(), &all);
}

#[test]
fn extended_datetime_wrong_carriers_and_earlier_profile_refuse_without_coercion() {
    let case = model::source(6);
    checked(&case.before, &case.rows);
    for value in [
        Value::Int64(0),
        Value::Text("2000-02-29".into()),
        Value::Binary(vec![0; 42]),
    ] {
        let requests = [RowInsert::new(vec![
            Value::Int32(10),
            Value::Int32(10),
            value,
        ])];
        assert!(
            original::capture(&case.before)
                .preview_ace16_insert_rows("Items", &requests, &mut original::budget(), &|| false)
                .is_err()
        );
    }
    let earlier = model::source(5);
    assert!(
        read_table(
            &mut Cursor::new(&earlier.before),
            CatalogScope::new(1),
            "Items",
            &mut original::budget(),
            &|| false
        )
        .is_err()
    );
    let requests = [RowInsert::new(additions()[0].values())];
    assert!(
        original::capture(&earlier.before)
            .preview_ace16_insert_rows("Items", &requests, &mut original::budget(), &|| false)
            .is_err()
    );
}

#[test]
fn extended_datetime_insert_honors_all_pools_and_actual_terminal_cancellation() {
    let case = model::source(6);
    let added = additions();
    let expected = model::expected(&case, &added);
    checked(&case.before, &case.rows);
    checked(
        &expected,
        &case
            .rows
            .iter()
            .cloned()
            .chain(added.iter().cloned())
            .collect::<Vec<_>>(),
    );
    let requests = added
        .iter()
        .map(|row| RowInsert::new(row.values()))
        .collect::<Vec<_>>();
    for mut budget in [
        ResourceBudget::new(0, 100_000, 100_000_000),
        ResourceBudget::new(256 * 1024 * 1024, 0, 100_000_000),
        ResourceBudget::new(256 * 1024 * 1024, 100_000, 0),
    ] {
        assert!(
            original::capture(&case.before)
                .preview_ace16_insert_rows("Items", &requests, &mut budget, &|| false)
                .is_err()
        );
    }
    let calls = Cell::new(0);
    let observe = || {
        calls.set(calls.get() + 1);
        false
    };
    let healthy = original::capture(&case.before)
        .preview_ace16_insert_rows("Items", &requests, &mut original::budget(), &observe)
        .unwrap();
    same_image(healthy.candidate_bytes(), &expected);
    let terminal = calls.get();
    assert!(terminal > 1);
    calls.set(0);
    let cancel = || {
        calls.set(calls.get() + 1);
        calls.get() == terminal
    };
    assert!(
        original::capture(&case.before)
            .preview_ace16_insert_rows("Items", &requests, &mut original::budget(), &cancel)
            .is_err()
    );
    assert_eq!(calls.get(), terminal);
}

//! NUMERIC secondary membership is independent of its ordinary row storage.
mod display_metadata;
mod model;
mod omitted_defaults;
mod two_decimal;

use super::model as original;
use crate::{EditError, IndexConstraintViolation, InspectError, RowInsert, WriteBlocker};
use model::{Case, PAGE, Row, Settings};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Currency, Decimal, Value},
};
use tablecraft_model::CatalogScope;

fn requests(case: &Case, rows: &[Row]) -> Vec<RowInsert> {
    rows.iter()
        .map(|row| RowInsert::new(row.values(case.settings)))
        .collect()
}

fn typed(bytes: &[u8], expected: &[Row], settings: Settings) {
    let table = crate::read_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut original::budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(table.rows().len(), expected.len());
    for (actual, row) in table.rows().iter().zip(expected) {
        assert_eq!(actual, &row.values(settings));
    }
}

fn run(case: &Case, added: &[Row]) -> Vec<u8> {
    original::checked(&case.before, case.rows.len(), 4);
    let mut rows = case
        .rows
        .iter()
        .map(|(row, _)| row.clone())
        .collect::<Vec<_>>();
    typed(&case.before, &rows, case.settings);
    // All expected bytes and readable semantic checks precede the preview call.
    let expected = model::expected(case, added);
    rows.extend_from_slice(added);
    original::checked(&expected, rows.len(), 4);
    typed(&expected, &rows, case.settings);
    let candidate = original::capture(&case.before)
        .preview_ace16_insert_rows(
            "Items",
            &requests(case, added),
            &mut original::budget(),
            &|| false,
        )
        .unwrap();
    let actual = candidate.candidate_bytes();
    assert_eq!(actual.len(), expected.len());
    let difference = actual
        .iter()
        .zip(&expected)
        .position(|(left, right)| left != right);
    assert!(
        difference.is_none(),
        "first Decimal full-image difference at {difference:?}"
    );
    original::checked(actual, rows.len(), 4);
    typed(actual, &rows, case.settings);
    expected
}

fn duplicate(case: &Case, row: Row) {
    original::checked(&case.before, case.rows.len(), 4);
    let result = original::capture(&case.before).preview_ace16_insert_rows(
        "Items",
        &requests(case, &[row]),
        &mut original::budget(),
        &|| false,
    );
    assert_eq!(
        result.err(),
        Some(EditError::Inspect(InspectError::IndexConstraint(
            IndexConstraintViolation::DuplicateKey,
        )))
    );
}

#[test]
fn decimal_secondary_values_keep_28_digits_exact_scales_zero_null_and_alias_counters() {
    for code in [5, 6] {
        for descending in [false, true] {
            for (precision, scale) in [(1, 0), (3, 2), (28, 4), (28, 28)] {
                let mut settings = Settings::ordinary(false, descending);
                settings.precision = precision;
                settings.scale = scale;
                let case = model::source(code, settings, false);
                let added = [
                    Row {
                        id: 10,
                        amount: Some(0),
                        guid: None,
                    },
                    Row {
                        id: 11,
                        amount: None,
                        guid: None,
                    },
                    Row {
                        id: 12,
                        amount: Some(0),
                        guid: None,
                    },
                ];
                let after = run(&case, &added);
                assert_eq!(
                    u32::from_le_bytes(after[10 * PAGE + 79..10 * PAGE + 83].try_into().unwrap()),
                    660
                );
                assert_eq!(&after[14 * PAGE..14 * PAGE + 2], &[4, 1]);
            }
        }
    }
}

#[test]
fn decimal_and_guid_components_use_written_order_mixed_directions_and_partial_null_keys() {
    for code in [5, 6] {
        for descending in [false, true] {
            for reversed in [false, true] {
                for ignored in [false, true] {
                    let mut settings = Settings::ordinary(true, descending);
                    settings.reversed = reversed;
                    settings.ignored = ignored;
                    let case = model::source(code, settings, false);
                    run(
                        &case,
                        &[
                            Row {
                                id: 10,
                                amount: Some(0),
                                guid: Some(model::guid(90)),
                            },
                            Row {
                                id: 11,
                                amount: None,
                                guid: Some(model::guid(91)),
                            },
                            Row {
                                id: 12,
                                amount: Some(1),
                                guid: None,
                            },
                            Row {
                                id: 13,
                                amount: None,
                                guid: None,
                            },
                        ],
                    );
                }
            }
        }
    }
}

#[test]
fn scalar_22_byte_and_guid_tuple_41_byte_entries_split_with_26_and_45_byte_bounds() {
    for code in [5, 6] {
        for tuple in [false, true] {
            for descending in [false, true] {
                for ignored in [false, true] {
                    let mut settings = Settings::ordinary(tuple, descending);
                    settings.ignored = ignored;
                    let case = model::source(code, settings, true);
                    let free = u16::from_le_bytes(
                        case.before[14 * PAGE + 2..14 * PAGE + 4]
                            .try_into()
                            .unwrap(),
                    );
                    assert_eq!(
                        free,
                        match (tuple, ignored) {
                            (false, _) => 3,
                            (true, false) => 2,
                            (true, true) => 15,
                        }
                    );
                    let after = run(
                        &case,
                        &[Row {
                            id: 1000,
                            amount: Some(0),
                            guid: tuple.then(|| model::guid(240)),
                        }],
                    );
                    assert_eq!(after.len(), case.before.len() + 2 * PAGE);
                    assert_eq!(&after[14 * PAGE..14 * PAGE + 2], &[3, 1]);
                    assert_eq!(
                        u16::from_le_bytes(after[14 * PAGE + 2..14 * PAGE + 4].try_into().unwrap()),
                        if tuple { 3571 } else { 3590 }
                    );
                    assert_eq!(
                        u32::from_le_bytes(
                            after[10 * PAGE + 79..10 * PAGE + 83].try_into().unwrap()
                        ),
                        660
                    );
                }
            }
        }
    }
}

#[test]
fn unique_decimal_keys_exempt_only_all_null_and_compare_the_complete_tuple() {
    for tuple in [false, true] {
        for ignored in [false, true] {
            let mut settings = Settings::ordinary(tuple, false);
            settings.unique = true;
            settings.ignored = ignored;
            let case = model::source(6, settings, false);
            run(
                &case,
                &[
                    Row {
                        id: 10,
                        amount: None,
                        guid: None,
                    },
                    Row {
                        id: 11,
                        amount: None,
                        guid: None,
                    },
                ],
            );
            let mut same = case.rows[0].0.clone();
            same.id = 10;
            duplicate(&case, same);
            if tuple {
                let row = Row {
                    id: 12,
                    amount: Some(0),
                    guid: None,
                };
                run(&case, std::slice::from_ref(&row));
                // Duplicate the partial-NULL new key against the immutable batch,
                // avoiding a synthetic source row inventory for the second call.
                let mut second = row.clone();
                second.id = 13;
                let result = original::capture(&case.before).preview_ace16_insert_rows(
                    "Items",
                    &requests(&case, &[row, second]),
                    &mut original::budget(),
                    &|| false,
                );
                assert_eq!(
                    result.err(),
                    Some(EditError::Inspect(InspectError::IndexConstraint(
                        IndexConstraintViolation::DuplicateKey
                    )))
                );
            }
        }
    }
}

#[test]
fn decimal_index_requests_never_rescale_truncate_or_coerce_and_required_rejects_null() {
    let mut settings = Settings::ordinary(false, false);
    settings.precision = 3;
    settings.scale = 2;
    let case = model::source(6, settings, false);
    original::checked(&case.before, case.rows.len(), 4);
    for bad in [
        Value::Decimal(Decimal::new(1, 1).unwrap()),
        Value::Decimal(Decimal::new(1000, 2).unwrap()),
        Value::Int32(1),
        Value::Currency(Currency::from_scaled(1)),
        Value::Binary(vec![0; 17]),
    ] {
        let result = original::capture(&case.before).preview_ace16_insert_rows(
            "Items",
            &[RowInsert::new(vec![Value::Int32(10), bad])],
            &mut original::budget(),
            &|| false,
        );
        assert_eq!(
            result.err(),
            Some(EditError::Unsupported(WriteBlocker::Column))
        );
    }
    settings.required = true;
    let case = model::source(6, settings, false);
    original::checked(&case.before, case.rows.len(), 4);
    let result = original::capture(&case.before).preview_ace16_insert_rows(
        "Items",
        &requests(
            &case,
            &[Row {
                id: 10,
                amount: None,
                guid: None,
            }],
        ),
        &mut original::budget(),
        &|| false,
    );
    assert!(result.is_err());
}

#[test]
fn ignored_decimal_all_null_batches_preserve_index_bytes_and_numeric_history() {
    for tuple in [false, true] {
        let mut settings = Settings::ordinary(tuple, true);
        settings.ignored = true;
        let case = model::source(6, settings, false);
        let after = run(
            &case,
            &[Row {
                id: 10,
                amount: None,
                guid: None,
            }],
        );
        assert_eq!(
            &after[14 * PAGE..15 * PAGE],
            &case.before[14 * PAGE..15 * PAGE]
        );
        assert_eq!(
            &after[10 * PAGE + 79..10 * PAGE + 83],
            &659_u32.to_le_bytes()
        );
    }
}

#[test]
fn decimal_index_work_honors_shared_quotas_and_late_cancellation_without_candidate() {
    let case = model::source(6, Settings::ordinary(true, false), true);
    original::checked(&case.before, case.rows.len(), 4);
    let request = requests(
        &case,
        &[Row {
            id: 1000,
            amount: Some(0),
            guid: Some(model::guid(240)),
        }],
    );
    for mut budget in [
        ResourceBudget::new(0, 100_000, 100_000_000),
        ResourceBudget::new(256 * 1024 * 1024, 0, 100_000_000),
        ResourceBudget::new(256 * 1024 * 1024, 100_000, 0),
    ] {
        assert!(
            original::capture(&case.before)
                .preview_ace16_insert_rows("Items", &request, &mut budget, &|| false)
                .is_err()
        );
    }
    let expected = model::expected(
        &case,
        &[Row {
            id: 1000,
            amount: Some(0),
            guid: Some(model::guid(240)),
        }],
    );
    let healthy_calls = Cell::new(0);
    let observe = || {
        healthy_calls.set(healthy_calls.get() + 1);
        false
    };
    let healthy = original::capture(&case.before)
        .preview_ace16_insert_rows("Items", &request, &mut original::budget(), &observe)
        .unwrap();
    let actual = healthy.candidate_bytes();
    assert_eq!(actual.len(), expected.len());
    let difference = actual
        .iter()
        .zip(&expected)
        .position(|(actual, expected)| actual != expected);
    assert!(
        difference.is_none(),
        "first healthy Decimal image difference at {difference:?}"
    );
    let terminal = healthy_calls.get();
    assert!(terminal > 1);
    let calls = Cell::new(0);
    let cancel = || {
        calls.set(calls.get() + 1);
        calls.get() == terminal
    };
    assert!(
        original::capture(&case.before)
            .preview_ace16_insert_rows("Items", &request, &mut original::budget(), &cancel)
            .is_err()
    );
    assert_eq!(calls.get(), terminal);
}

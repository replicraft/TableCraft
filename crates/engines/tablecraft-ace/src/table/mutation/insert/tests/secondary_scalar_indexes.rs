//! Bit-exact scalar secondary keys over genuine mask/fixed records and aliases.
mod model;

use super::model as original;
use crate::{EditError, IndexConstraintViolation, InspectError, RowInsert, WriteBlocker};
use model::{Case, Kind, Row, Settings};
use std::cell::Cell;
use tablecraft_core::{
    limits::{LimitError, Resource, ResourceBudget},
    values::{Double, Single, Value},
};

const PAGE: usize = 4096;

fn requests(case: &Case, rows: &[Row]) -> Vec<RowInsert> {
    rows.iter()
        .map(|row| RowInsert::new(row.values(case.settings.kind)))
        .collect()
}

fn run(case: &Case, rows: &[Row]) -> Vec<u8> {
    original::checked(&case.before, case.rows.len(), 4);
    model::assert_typed(&case.before, &case.rows, case.settings.kind);
    let expected = model::expected(case, rows);
    original::checked(&expected, case.rows.len() + rows.len(), 4);
    let candidate = original::capture(&case.before)
        .preview_ace16_insert_rows(
            "Items",
            &requests(case, rows),
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
        "first byte difference at {difference:?}"
    );
    original::checked(actual, case.rows.len() + rows.len(), 4);
    let mut expected_rows = case.rows.clone();
    expected_rows.extend(rows.iter().cloned().map(|row| (row, 0)));
    model::assert_typed(actual, &expected_rows, case.settings.kind);
    expected
}

fn rows(case: &Case, values: &[Option<u64>]) -> Vec<Row> {
    values
        .iter()
        .copied()
        .enumerate()
        .map(|(position, group)| Row {
            id: (case.rows.len() + position + 1) as i32,
            group,
        })
        .collect()
}

fn reject(case: &Case, value: Value, error: EditError) {
    original::checked(&case.before, case.rows.len(), 4);
    let request = RowInsert::new(vec![
        Value::Int32(case.rows.len() as i32 + 1),
        value,
        Value::Binary(Vec::new()),
    ]);
    assert_eq!(
        original::capture(&case.before)
            .preview_ace16_insert_rows("Items", &[request], &mut original::budget(), &|| false)
            .err(),
        Some(error)
    );
}

#[test]
fn boolean_mask_values_and_ignored_flag_preserve_complete_nonnull_membership() {
    for code in [5, 6] {
        for descending in [false, true] {
            for ignored in [false, true] {
                let settings = Settings {
                    ignore_nulls: ignored,
                    ..Settings::ordinary(Kind::Boolean, descending)
                };
                let case = model::source(code, settings, false);
                let inserted = rows(&case, &[Some(0), Some(1), Some(0)]);
                let after = run(&case, &inserted);
                assert_eq!(
                    &after[10 * PAGE + 79..10 * PAGE + 83],
                    &29_u32.to_le_bytes()
                );
                reject(
                    &case,
                    Value::Null,
                    EditError::Unsupported(WriteBlocker::Column),
                );
                reject(
                    &case,
                    Value::Byte(0),
                    EditError::Unsupported(WriteBlocker::Column),
                );
            }
        }
    }
}

#[test]
fn ieee_secondary_keys_preserve_zero_subnormal_infinity_and_nan_payload_bits() {
    for kind in [Kind::Single, Kind::Double] {
        let bits: &[u64] = if kind == Kind::Single {
            &[
                0x00000001, 0x80000001, 0x7f7fffff, 0xff7fffff, 0x7f800000, 0xff800000, 0x7fc00001,
                0x7fc00002, 0xffc00001, 0x7f800001,
            ]
        } else {
            &[
                0x0000000000000001,
                0x8000000000000001,
                0x7fefffffffffffff,
                0xffefffffffffffff,
                0x7ff0000000000000,
                0xfff0000000000000,
                0x7ff8000000000001,
                0x7ff8000000000002,
                0xfff8000000000001,
                0x7ff0000000000001,
            ]
        };
        for code in [5, 6] {
            for descending in [false, true] {
                let case = model::source(code, Settings::ordinary(kind, descending), false);
                let mut values = bits.iter().copied().map(Some).collect::<Vec<_>>();
                values.extend([Some(0), Some(bits[6]), None]);
                let after = run(&case, &rows(&case, &values));
                assert_eq!(
                    &after[10 * PAGE + 79..10 * PAGE + 83],
                    &39_u32.to_le_bytes()
                );
            }
        }
    }
}

#[test]
fn legacy_date_keys_keep_signed_zeros_calendar_aliases_and_submillisecond_bits() {
    for code in [5, 6] {
        for descending in [false, true] {
            let case = model::source(code, Settings::ordinary(Kind::Date, descending), false);
            let inserted = rows(
                &case,
                &[
                    Some(0xbfd0000000000000),
                    Some(0x3fd0000000000000),
                    Some(0xbff4000000000000),
                    Some(0x3ff4000000000000),
                    Some(0x3ff0000000000001),
                    Some(0x3ff4000000000000),
                    None,
                ],
            );
            let after = run(&case, &inserted);
            assert_eq!(
                &after[10 * PAGE + 79..10 * PAGE + 83],
                &34_u32.to_le_bytes()
            );
        }
    }
}

#[test]
fn unique_physical_keys_reject_exact_duplicates_but_keep_distinct_ieee_families() {
    for kind in [Kind::Boolean, Kind::Single, Kind::Double, Kind::Date] {
        for code in [5, 6] {
            for descending in [false, true] {
                let settings = Settings {
                    unique: true,
                    ..Settings::ordinary(kind, descending)
                };
                let case = model::source(code, settings, false);
                let duplicate = case.rows[0].0.values(kind)[1].clone();
                reject(
                    &case,
                    duplicate,
                    EditError::Inspect(InspectError::IndexConstraint(
                        IndexConstraintViolation::DuplicateKey,
                    )),
                );
                if kind != Kind::Boolean {
                    let first = if kind == Kind::Single {
                        0x7fc00001
                    } else if kind == Kind::Double {
                        0x7ff8000000000001
                    } else {
                        0x3ff4000000000000
                    };
                    let second = first + 1;
                    let after = run(
                        &case,
                        &rows(&case, &[Some(first), Some(second), None, None]),
                    );
                    assert_eq!(
                        &after[10 * PAGE + 79..10 * PAGE + 83],
                        &31_u32.to_le_bytes()
                    );
                    let repeated = rows(&case, &[Some(first), Some(first)]);
                    assert_eq!(
                        original::capture(&case.before)
                            .preview_ace16_insert_rows(
                                "Items",
                                &requests(&case, &repeated),
                                &mut original::budget(),
                                &|| false
                            )
                            .err(),
                        Some(EditError::Inspect(InspectError::IndexConstraint(
                            IndexConstraintViolation::DuplicateKey
                        )))
                    );
                }
            }
        }
    }
}

#[test]
fn ignored_float_and_date_nulls_have_zero_index_effect_and_nonnull_family_deltas() {
    for kind in [Kind::Single, Kind::Double, Kind::Date] {
        for code in [5, 6] {
            for descending in [false, true] {
                let settings = Settings {
                    ignore_nulls: true,
                    ..Settings::ordinary(kind, descending)
                };
                let case = model::source(code, settings, false);
                let after = run(&case, &rows(&case, &[None, None]));
                assert_eq!(
                    &after[14 * PAGE..16 * PAGE],
                    &case.before[14 * PAGE..16 * PAGE]
                );
                assert_eq!(
                    &after[10 * PAGE + 79..10 * PAGE + 83],
                    &29_u32.to_le_bytes()
                );
                let one = if kind == Kind::Single {
                    0x3f800000
                } else {
                    0x3ff0000000000000
                };
                let after = run(&case, &rows(&case, &[None, Some(one), Some(one), Some(0)]));
                assert_eq!(
                    &after[10 * PAGE + 79..10 * PAGE + 83],
                    &30_u32.to_le_bytes()
                );
            }
        }
    }
}

#[test]
fn nullable_scalar_secondary_root_growth_preserves_primary_and_shared_cursor() {
    for kind in [Kind::Single, Kind::Double, Kind::Date] {
        for code in [5, 6] {
            for descending in [false, true] {
                let case = model::source(code, Settings::ordinary(kind, descending), true);
                let values = if kind == Kind::Single {
                    vec![Some(kind.negative_zero()), Some(0)]
                } else {
                    vec![Some(kind.negative_zero())]
                };
                let after = run(&case, &rows(&case, &values));
                assert_eq!(after.len(), 20 * PAGE);
                assert_eq!(after[13 * PAGE], 4);
                assert_eq!(after[14 * PAGE], 3);
                let free = if kind == Kind::Single {
                    3603_u16
                } else {
                    3599_u16
                };
                assert_eq!(&after[14 * PAGE + 2..14 * PAGE + 4], &free.to_le_bytes());
                assert_eq!(
                    &after[14 * PAGE + 20..14 * PAGE + 24],
                    &18_u32.to_le_bytes()
                );
                assert_eq!(
                    &after[18 * PAGE + 12..18 * PAGE + 16],
                    &19_u32.to_le_bytes()
                );
                assert_eq!(
                    &after[19 * PAGE + 16..19 * PAGE + 20],
                    &18_u32.to_le_bytes()
                );
            }
        }
    }
}

#[test]
fn required_ignored_combinations_and_wrong_scalar_carriers_stay_refused() {
    for kind in [Kind::Boolean, Kind::Single, Kind::Double, Kind::Date] {
        let settings = Settings {
            required: true,
            ignore_nulls: true,
            ..Settings::ordinary(kind, false)
        };
        let case = model::source(6, settings, false);
        let value = case.rows[0].0.values(kind)[1].clone();
        reject(
            &case,
            value,
            EditError::Unsupported(WriteBlocker::IndexedColumn),
        );
    }
    let single = model::source(6, Settings::ordinary(Kind::Single, false), false);
    reject(
        &single,
        Value::Double(Double::from_bits(0)),
        EditError::Unsupported(WriteBlocker::Column),
    );
    let double = model::source(6, Settings::ordinary(Kind::Double, false), false);
    reject(
        &double,
        Value::Single(Single::from_bits(0)),
        EditError::Unsupported(WriteBlocker::Column),
    );
    let date = model::source(6, Settings::ordinary(Kind::Date, false), false);
    reject(
        &date,
        Value::Double(Double::from_bits(0)),
        EditError::Unsupported(WriteBlocker::Column),
    );
}

#[test]
fn scalar_secondary_work_charges_all_pools_and_late_cancellation_returns_no_candidate() {
    let settings = Settings {
        ignore_nulls: true,
        ..Settings::ordinary(Kind::Single, false)
    };
    let case = model::source(6, settings, false);
    let inserted = rows(&case, &[None, Some(0x3f800000)]);
    let expected = model::expected(&case, &inserted);
    original::checked(&case.before, 3, 4);
    original::checked(&expected, 5, 4);
    for (mut budget, resource) in [
        (
            ResourceBudget::new(1, 100_000, 100_000_000),
            Resource::Bytes,
        ),
        (
            ResourceBudget::new(256 * 1024 * 1024, 1, 100_000_000),
            Resource::Rows,
        ),
        (
            ResourceBudget::new(256 * 1024 * 1024, 100_000, 1),
            Resource::Steps,
        ),
    ] {
        let failure = original::capture(&case.before)
            .preview_ace16_insert_rows("Items", &requests(&case, &inserted), &mut budget, &|| false)
            .err();
        assert!(matches!(
            failure,
            Some(EditError::Inspect(InspectError::Limit(LimitError {
                resource: actual,
                ..
            }))) if actual == resource
        ));
    }
    let polls = Cell::new(0_usize);
    let healthy = original::capture(&case.before)
        .preview_ace16_insert_rows(
            "Items",
            &requests(&case, &inserted),
            &mut original::budget(),
            &|| {
                polls.set(polls.get() + 1);
                false
            },
        )
        .unwrap();
    assert_eq!(healthy.candidate_bytes().len(), expected.len());
    let difference = healthy
        .candidate_bytes()
        .iter()
        .zip(&expected)
        .position(|(left, right)| left != right);
    assert!(
        difference.is_none(),
        "first byte difference at {difference:?}"
    );
    let threshold = polls.get() - 1;
    polls.set(0);
    assert!(matches!(
        original::capture(&case.before)
            .preview_ace16_insert_rows(
                "Items",
                &requests(&case, &inserted),
                &mut original::budget(),
                &|| {
                    polls.set(polls.get() + 1);
                    polls.get() >= threshold
                }
            )
            .err(),
        Some(EditError::Inspect(InspectError::Cancelled))
    ));
}

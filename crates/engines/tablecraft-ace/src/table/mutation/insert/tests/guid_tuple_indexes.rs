//! Complete row/image controls independent of the typed GUID/tuple planner.
pub(in crate::table::mutation::insert) mod model;
mod wide;

use super::model as original;
use crate::{EditError, IndexConstraintViolation, InspectError, RowInsert, WriteBlocker};
use model::{Case, Row, Settings, Shape};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::{
    limits::{LimitError, Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;

const PAGE: usize = 4096;

fn requests(rows: &[Row]) -> Vec<RowInsert> {
    rows.iter()
        .map(|row| RowInsert::new(row.values()))
        .collect()
}

fn typed(bytes: &[u8], rows: &[Row]) {
    let snapshot = crate::read_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut original::budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(snapshot.rows().len(), rows.len());
    for (actual, row) in snapshot.rows().iter().zip(rows) {
        assert_eq!(actual, &row.values());
    }
}

fn run(case: &Case, inserted: &[Row]) -> Vec<u8> {
    original::checked(&case.before, case.rows.len(), 4);
    let mut rows = case
        .rows
        .iter()
        .map(|(row, _)| row.clone())
        .collect::<Vec<_>>();
    typed(&case.before, &rows);

    let expected = model::expected(case, inserted);
    rows.extend_from_slice(inserted);
    original::checked(&expected, rows.len(), 4);
    typed(&expected, &rows);

    let candidate = original::capture(&case.before)
        .preview_ace16_insert_rows(
            "Items",
            &requests(inserted),
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
        "first complete-image difference at {difference:?}"
    );
    original::checked(actual, rows.len(), 4);
    typed(actual, &rows);
    expected
}

fn duplicate(case: &Case, inserted: &[Row]) {
    original::checked(&case.before, case.rows.len(), 4);
    let failure = original::capture(&case.before)
        .preview_ace16_insert_rows(
            "Items",
            &requests(inserted),
            &mut original::budget(),
            &|| false,
        )
        .err();
    assert_eq!(
        failure,
        Some(EditError::Inspect(InspectError::IndexConstraint(
            IndexConstraintViolation::DuplicateKey,
        )))
    );
}

#[test]
fn guid_secondary_keys_keep_canonical_values_segments_aliases_and_primary_ownership() {
    for code in [5, 6] {
        for descending in [false, true] {
            for primary_second in [false, true] {
                let settings = Settings {
                    guid_descending: descending,
                    primary_second,
                    ..Settings::ordinary(Shape::Guid)
                };
                let case = model::source(code, settings, false);
                let inserted = [
                    Row {
                        id: 4,
                        guid: Some(model::guid(128)),
                        score: Some(-7),
                    },
                    Row {
                        id: 5,
                        guid: None,
                        score: Some(123),
                    },
                ];
                run(&case, &inserted);
            }
        }
    }
}

#[test]
fn mixed_direction_tuples_compare_all_components_in_written_key_order() {
    for shape in [Shape::GuidThenScore, Shape::ScoreThenGuid] {
        for code in [5, 6] {
            for guid_descending in [false, true] {
                for score_descending in [false, true] {
                    let settings = Settings {
                        guid_descending,
                        score_descending,
                        ..Settings::ordinary(shape)
                    };
                    let case = model::source(code, settings, false);
                    let inserted = [
                        Row {
                            id: 4,
                            guid: Some(model::guid(16)),
                            score: Some(-7),
                        },
                        Row {
                            id: 5,
                            guid: None,
                            score: Some(0),
                        },
                        Row {
                            id: 6,
                            guid: Some(model::guid(128)),
                            score: None,
                        },
                    ];
                    run(&case, &inserted);
                }
            }
        }
    }
}

#[test]
fn ignored_tuple_membership_omits_only_all_null_selected_components() {
    for shape in [Shape::Guid, Shape::GuidThenScore, Shape::ScoreThenGuid] {
        for code in [5, 6] {
            let settings = Settings {
                ignored: true,
                ..Settings::ordinary(shape)
            };
            let case = model::source(code, settings, false);
            let all_null = [
                Row {
                    id: 4,
                    guid: None,
                    score: None,
                },
                Row {
                    id: 5,
                    guid: None,
                    score: None,
                },
            ];
            let after = run(&case, &all_null);
            assert_eq!(
                &after[14 * PAGE..16 * PAGE],
                &case.before[14 * PAGE..16 * PAGE]
            );
            assert_eq!(
                &after[10 * PAGE + 79..10 * PAGE + 83],
                &29_u32.to_le_bytes()
            );

            let partial = [
                Row {
                    id: 4,
                    guid: None,
                    score: Some(10),
                },
                Row {
                    id: 5,
                    guid: Some(model::guid(32)),
                    score: None,
                },
            ];
            run(&case, &partial);
        }
    }
}

#[test]
fn unique_tuples_exempt_all_null_but_enforce_partial_null_and_full_key_duplicates() {
    for shape in [Shape::GuidThenScore, Shape::ScoreThenGuid] {
        for code in [5, 6] {
            for ignored in [false, true] {
                let settings = Settings {
                    unique: true,
                    ignored,
                    ..Settings::ordinary(shape)
                };
                let case = model::source(code, settings, false);
                run(
                    &case,
                    &[
                        Row {
                            id: 4,
                            guid: Some(model::guid(16)),
                            score: Some(30),
                        },
                        Row {
                            id: 5,
                            guid: None,
                            score: None,
                        },
                        Row {
                            id: 6,
                            guid: None,
                            score: None,
                        },
                    ],
                );
                duplicate(
                    &case,
                    &[Row {
                        id: 4,
                        guid: Some(model::guid(16)),
                        score: Some(10),
                    }],
                );
                duplicate(
                    &case,
                    &[
                        Row {
                            id: 4,
                            guid: None,
                            score: Some(99),
                        },
                        Row {
                            id: 5,
                            guid: None,
                            score: Some(99),
                        },
                    ],
                );
            }
        }
    }
}

#[test]
fn full_guid_leaf_growth_uses_wide_bounds_and_the_same_physical_allocator() {
    for code in [5, 6] {
        for guid_descending in [false, true] {
            for primary_second in [false, true] {
                let settings = Settings {
                    guid_descending,
                    primary_second,
                    ..Settings::ordinary(Shape::Guid)
                };
                let case = model::source(code, settings, true);
                let secondary = 1 - usize::from(primary_second);
                assert_eq!(
                    &case.before[(13 + secondary) * PAGE + 2..(13 + secondary) * PAGE + 4],
                    &[0, 0]
                );
                let after = run(
                    &case,
                    &[Row {
                        id: 159,
                        guid: Some(model::guid(250)),
                        score: Some(-1),
                    }],
                );
                assert_eq!(after.len(), 20 * PAGE);
                assert_eq!(after[(13 + secondary) * PAGE], 3);
                assert_eq!(after[(13 + usize::from(primary_second)) * PAGE], 4);
            }
        }
    }
}

#[test]
fn required_tuple_components_and_exact_guid_carriers_are_not_coerced() {
    for code in [5, 6] {
        let settings = Settings {
            required: true,
            ..Settings::ordinary(Shape::GuidThenScore)
        };
        let case = model::source(code, settings, false);
        original::checked(&case.before, 3, 4);

        let partial = [Row {
            id: 4,
            guid: Some(model::guid(64)),
            score: None,
        }];
        let failure = original::capture(&case.before)
            .preview_ace16_insert_rows(
                "Items",
                &requests(&partial),
                &mut original::budget(),
                &|| false,
            )
            .err();
        assert_eq!(
            failure,
            Some(EditError::Inspect(InspectError::IndexConstraint(
                IndexConstraintViolation::NullKey,
            )))
        );

        let wrong = RowInsert::new(vec![
            Value::Int32(4),
            Value::Binary(model::guid(64).to_vec()),
            Value::Int32(40),
        ]);
        let failure = original::capture(&case.before)
            .preview_ace16_insert_rows("Items", &[wrong], &mut original::budget(), &|| false)
            .err();
        assert_eq!(failure, Some(EditError::Unsupported(WriteBlocker::Column)));
    }
}

#[test]
fn wider_key_preparation_and_audit_keep_resource_pools_and_terminal_cancellation() {
    let case = model::source(6, Settings::ordinary(Shape::ScoreThenGuid), false);
    let inserted = [Row {
        id: 4,
        guid: Some(model::guid(128)),
        score: Some(40),
    }];
    let expected = run(&case, &inserted);
    let requests = requests(&inserted);

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
            .preview_ace16_insert_rows("Items", &requests, &mut budget, &|| false)
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
        .preview_ace16_insert_rows("Items", &requests, &mut original::budget(), &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap();
    assert_eq!(healthy.candidate_bytes(), expected);

    let last = polls.get();
    assert!(last > 1);
    polls.set(0);
    let failure = original::capture(&case.before)
        .preview_ace16_insert_rows("Items", &requests, &mut original::budget(), &|| {
            polls.set(polls.get() + 1);
            polls.get() == last
        })
        .err();
    assert_eq!(failure, Some(EditError::Inspect(InspectError::Cancelled)));
}

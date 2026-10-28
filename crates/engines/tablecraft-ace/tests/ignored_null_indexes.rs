#[allow(dead_code)]
#[path = "mutation/fixture.rs"]
mod base;
#[path = "ignored_null_indexes/fixture.rs"]
mod ignored;
#[allow(dead_code)]
#[path = "mutation/index_fixture.rs"]
mod index_fixture;
use tablecraft_testkit::access as support;

use std::{cell::Cell, io::Cursor, ops::Bound};
use tablecraft_ace::{
    EditError, IndexCheckStatus, IndexConstraintViolation, IndexProjection,
    IndexProjectionSelection, InspectError, Int32IndexSelection, Int32Update, NativeImage,
    TableCheck, WriteBlocker, check_table, read_table_index, read_table_index_projection,
    read_table_index_range,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;

// The shared mutation fixture calls its sibling "fixture".
use base as fixture;

const SCOPE: CatalogScope = CatalogScope::new(1);
const VALUES: &[Option<i32>] = &[Some(20), None, Some(10), Some(30), None, Some(20)];

fn budget() -> ResourceBudget {
    ResourceBudget::new(32 * 1024 * 1024, 16_384, 2_000_000)
}

fn check(bytes: &[u8]) -> Result<TableCheck, InspectError> {
    check_table(
        &mut Cursor::new(bytes),
        SCOPE,
        "Items",
        &mut budget(),
        &|| false,
    )
}

#[test]
fn complete_checks_count_only_eligible_int32_roots_but_keep_the_full_table_population() {
    for code in [1, 2, 5, 6] {
        for descending in [false, true] {
            for node in [false, true] {
                let bytes = ignored::database(code, descending, false, node, VALUES);
                let original = bytes.clone();
                let report = check(&bytes).unwrap();
                assert_eq!(report.rows(), 6);
                assert_eq!(report.schema().declared_rows(), 6);
                assert_eq!(report.indexes().len(), 2);
                assert!(
                    report
                        .indexes()
                        .iter()
                        .all(|index| index.status() == IndexCheckStatus::Checked)
                );
                assert_eq!(bytes, original);
            }
        }
        for values in [&[][..], &[None, None][..], &[Some(10), Some(20)][..]] {
            let bytes = ignored::database(code, false, true, false, values);
            assert_eq!(check(&bytes).unwrap().rows(), values.len());
        }
    }
}

#[test]
fn non_null_ranges_and_equality_seek_both_directions_without_becoming_complete_table_reads() {
    for code in [1, 2, 5, 6] {
        for descending in [false, true] {
            for node in [false, true] {
                let bytes = ignored::database(code, descending, false, node, VALUES);
                for (lower, upper) in [
                    (Bound::Included(20), Bound::Included(20)),
                    (Bound::Unbounded, Bound::Unbounded),
                    (Bound::Excluded(30), Bound::Unbounded),
                    (Bound::Included(30), Bound::Included(10)),
                ] {
                    let selection = Int32IndexSelection::Range { lower, upper };
                    let actual = read_table_index_range(
                        &mut Cursor::new(&bytes),
                        SCOPE,
                        "Items",
                        "ScoreAlias",
                        selection,
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                    let mut expected: Vec<_> = VALUES
                        .iter()
                        .enumerate()
                        .filter_map(|(slot, value)| {
                            let value = (*value)?;
                            let above_lower = match lower {
                                Bound::Included(low) => value >= low,
                                Bound::Excluded(low) => value > low,
                                Bound::Unbounded => true,
                            };
                            let below_upper = match upper {
                                Bound::Included(high) => value <= high,
                                Bound::Excluded(high) => value < high,
                                Bound::Unbounded => true,
                            };
                            (above_lower && below_upper).then_some((value, slot as i32 + 1))
                        })
                        .collect();
                    expected.sort_by(|(a, left), (b, right)| {
                        let order = a.cmp(b);
                        (if descending { order.reverse() } else { order }).then(left.cmp(right))
                    });
                    let expected: Vec<_> = expected
                        .into_iter()
                        .map(|(value, id)| vec![Value::Int32(id), Value::Int32(value)])
                        .collect();
                    assert_eq!(actual.rows(), expected);
                    assert_eq!(actual.schema().declared_rows(), 6);
                }
                for selection in [Int32IndexSelection::All, Int32IndexSelection::Null] {
                    assert_eq!(
                        read_table_index_range(
                            &mut Cursor::new(&bytes),
                            SCOPE,
                            "Items",
                            "ScoreKey",
                            selection,
                            &mut budget(),
                            &|| false
                        )
                        .err(),
                        Some(InspectError::Unsupported)
                    );
                }
                assert_eq!(
                    read_table_index(
                        &mut Cursor::new(&bytes),
                        SCOPE,
                        "Items",
                        "ScoreKey",
                        &mut budget(),
                        &|| false
                    )
                    .err(),
                    Some(InspectError::Unsupported)
                );
            }
        }
    }
}

#[test]
fn selected_projection_still_decodes_the_key_and_refuses_missing_extra_or_null_entries() {
    for code in [1, 2, 5, 6] {
        let original = ignored::database(code, false, false, false, VALUES);
        let selected = || IndexProjection {
            columns: &["Id"],
            selection: IndexProjectionSelection::Int32(Int32IndexSelection::Range {
                lower: Bound::Included(20),
                upper: Bound::Included(20),
            }),
        };
        let result = read_table_index_projection(
            &mut Cursor::new(&original),
            SCOPE,
            "Items",
            "ScoreKey",
            selected(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            result.rows(),
            [vec![Value::Int32(1)], vec![Value::Int32(6)]]
        );
        for (entries, expected) in [
            (
                vec![
                    ignored::key(Some(10), 2, false),
                    ignored::key(Some(20), 0, false),
                    ignored::key(Some(30), 3, false),
                ],
                InspectError::Malformed,
            ),
            (
                vec![
                    ignored::key(None, 1, false),
                    ignored::key(Some(10), 2, false),
                    ignored::key(Some(20), 0, false),
                    ignored::key(Some(30), 3, false),
                ],
                InspectError::Malformed,
            ),
            (
                vec![
                    ignored::key(Some(10), 2, false),
                    ignored::key(Some(20), 0, false),
                    ignored::key(Some(20), 0, false),
                    ignored::key(Some(30), 3, false),
                ],
                InspectError::Malformed,
            ),
        ] {
            let mut bytes = original.clone();
            bytes[ignored::ROOT * ignored::PAGE_BYTES..(ignored::ROOT + 1) * ignored::PAGE_BYTES]
                .copy_from_slice(&ignored::page(true, &entries, 0, 0, 0));
            assert_eq!(check(&bytes).err(), Some(expected));
        }
        let mut wrong_key = original.clone();
        wrong_key[ignored::ROW_PAGE * ignored::PAGE_BYTES + ignored::PAGE_BYTES - 5
            ..ignored::ROW_PAGE * ignored::PAGE_BYTES + ignored::PAGE_BYTES - 1]
            .copy_from_slice(&99_i32.to_le_bytes());
        assert!(
            read_table_index_projection(
                &mut Cursor::new(&wrong_key),
                SCOPE,
                "Items",
                "ScoreKey",
                selected(),
                &mut budget(),
                &|| false
            )
            .is_err()
        );
    }
}

#[test]
fn ignored_null_unique_and_required_declarations_do_not_hide_violations() {
    let duplicate = ignored::database(5, false, true, false, VALUES);
    assert_eq!(
        check(&duplicate).err(),
        Some(InspectError::IndexConstraint(
            IndexConstraintViolation::DuplicateKey
        ))
    );
    let mut required = ignored::database(5, false, false, false, VALUES);
    required[ignored::INDEX_FLAGS] = 0x8a;
    assert_eq!(
        check(&required).err(),
        Some(InspectError::IndexConstraint(
            IndexConstraintViolation::NullKey
        ))
    );
}

#[test]
fn every_check_checkpoint_can_cancel_and_small_work_budgets_publish_no_report() {
    let bytes = ignored::database(5, false, false, true, VALUES);
    let observed = Cell::new(0);
    check_table(
        &mut Cursor::new(&bytes),
        SCOPE,
        "Items",
        &mut budget(),
        &|| {
            observed.set(observed.get() + 1);
            false
        },
    )
    .unwrap();
    for stop in 1..=observed.get() {
        let calls = Cell::new(0);
        let actual = check_table(
            &mut Cursor::new(&bytes),
            SCOPE,
            "Items",
            &mut budget(),
            &|| {
                calls.set(calls.get() + 1);
                calls.get() == stop
            },
        );
        assert_eq!(
            actual.err(),
            Some(InspectError::Cancelled),
            "checkpoint {stop}"
        );
    }
    for resource in [Resource::Bytes, Resource::Rows, Resource::Steps] {
        let mut allowance = budget();
        allowance
            .charge(resource, allowance.remaining(resource))
            .unwrap();
        assert!(matches!(
            check_table(
                &mut Cursor::new(&bytes),
                SCOPE,
                "Items",
                &mut allowance,
                &|| false
            ),
            Err(InspectError::Limit(_))
        ));
    }
    assert_eq!(check(&bytes).unwrap().rows(), 6);
}

#[test]
fn newly_checked_ignore_null_indexes_do_not_expand_normal_jet4_mutation_eligibility() {
    let bytes = ignored::database(1, false, false, false, VALUES);
    for updates in [Vec::new(), vec![Int32Update::new(0, 1, 9)]] {
        let image = NativeImage::capture(&mut Cursor::new(&bytes), SCOPE, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(
            image
                .plan_int32_updates("Items", "Id", &updates, &mut budget(), &|| false)
                .err(),
            Some(EditError::Unsupported(WriteBlocker::ConstraintCoverage))
        );
    }
}

#[cfg(feature = "ace16-write-preview")]
#[test]
fn newly_checked_ignore_null_indexes_do_not_expand_ace16_engineering_preview_eligibility() {
    for code in [5, 6] {
        let bytes = ignored::database(code, false, false, false, VALUES);
        for updates in [Vec::new(), vec![Int32Update::new(0, 1, 9)]] {
            let image =
                NativeImage::capture(&mut Cursor::new(&bytes), SCOPE, &mut budget(), &|| false)
                    .unwrap();
            assert_eq!(
                image
                    .preview_ace16_int32_updates("Items", "Id", &updates, &mut budget(), &|| false)
                    .err(),
                Some(EditError::Unsupported(WriteBlocker::ConstraintCoverage))
            );
        }
    }
}

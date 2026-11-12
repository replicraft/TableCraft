//! The normal CREATE producer's actual output is writable without a source fiction.
use crate::{FittedColumnUpdates, FixedScalarUpdate, NativeImage, SecondaryIndexCreate};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{
    raw_forwarding_chain,
    secondary_index::normal::{Population, updating as raw},
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(8 * 1024 * 1024 * 1024, 30_000_000, 16_000_000_000)
}
fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture_retained(
        &mut Cursor::new(bytes),
        CatalogScope::new(32610),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}
fn same(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    assert_eq!(
        actual.iter().zip(expected).position(|(a, b)| a != b),
        None,
        "first differing full image byte"
    );
}
fn updates(case: &raw::Case) -> Vec<FixedScalarUpdate> {
    case.cells
        .iter()
        .map(|cell| {
            FixedScalarUpdate::new(
                cell.row,
                cell.expected.map_or(Value::Null, Value::Int32),
                cell.value.map_or(Value::Null, Value::Int32),
            )
        })
        .collect()
}

#[test]
fn actual_create_then_indexed_update_preserves_full_images_and_retained_topology() {
    for descending in [false, true] {
        for case in [
            raw::source(Population::Empty, descending),
            raw::source(Population::NullableDuplicate, descending),
            raw::source(Population::ThreeLeaves, descending),
            raw::source(Population::MultiLevel, descending),
            raw::many_selected(descending),
            raw::protected_companion(descending),
        ] {
            let source = &case.creation;
            let created = image(&source.before)
                .prepare_retained_long_index_creation(
                    source.table,
                    &SecondaryIndexCreate::new(source.index_name, source.column, descending),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            same(created.candidate_bytes(), &source.expected);
            let requests = updates(&case);
            let groups = [FittedColumnUpdates::Fixed {
                column: source.column,
                updates: &requests,
            }];
            let plan = image(created.candidate_bytes())
                .plan_retained_indexed_long_updates(source.table, &groups, &mut budget(), &|| false)
                .unwrap_or_else(|error| {
                    panic!(
                        "rows={}, requests={}, levels={}, descending={descending}: {error:?}",
                        source.rows.len(),
                        case.cells.len(),
                        source.levels,
                    )
                });
            same(plan.before_bytes(), &source.expected);
            same(plan.after_bytes(), &case.expected);
            assert_eq!(
                plan.changed_rows(),
                case.cells
                    .iter()
                    .filter(|cell| cell.expected != cell.value)
                    .count()
            );
        }
        let original = raw::protected_companion(descending);
        let source = &original.creation;
        let created = image(&source.before)
            .prepare_retained_long_index_creation(
                source.table,
                &SecondaryIndexCreate::new(source.index_name, source.column, descending),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        same(created.candidate_bytes(), &source.expected);
        let forwarded = raw_forwarding_chain::extend(
            created.candidate_bytes(),
            source.target_definition + 1,
            0,
            20,
        );
        let case = raw::forwarded(descending);
        same(&forwarded, &case.creation.expected);
        let requests = updates(&case);
        let groups = [FittedColumnUpdates::Fixed {
            column: source.column,
            updates: &requests,
        }];
        let plan = image(&forwarded)
            .plan_retained_indexed_long_updates(source.table, &groups, &mut budget(), &|| false)
            .unwrap();
        same(plan.after_bytes(), &case.expected);
        assert_eq!(plan.changed_rows(), 3);
    }
}

#[test]
fn indexed_zero_single_noop_and_complete_sql_expression_are_not_cardinality_shortcuts() {
    for descending in [false, true] {
        for case in [
            raw::incremented(descending),
            raw::protected_incremented(descending),
            raw::single(descending),
        ] {
            let source = &case.creation;
            let requests = updates(&case);
            for selected in [&requests[..0], &requests[..1], requests.as_slice()] {
                let groups = [FittedColumnUpdates::Fixed {
                    column: source.column,
                    updates: selected,
                }];
                let plan = image(&source.expected)
                    .plan_retained_indexed_long_updates(
                        source.table,
                        &groups,
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                if selected.is_empty() {
                    same(plan.after_bytes(), &source.expected);
                    assert_eq!(plan.changed_rows(), 0);
                } else if selected.len() == requests.len() {
                    same(plan.after_bytes(), &case.expected);
                } else {
                    assert_eq!(plan.changed_rows(), 1);
                }
            }
            let noop = case
                .cells
                .iter()
                .map(|cell| {
                    FixedScalarUpdate::new(
                        cell.row,
                        cell.expected.map_or(Value::Null, Value::Int32),
                        cell.expected.map_or(Value::Null, Value::Int32),
                    )
                })
                .collect::<Vec<_>>();
            let groups = [FittedColumnUpdates::Fixed {
                column: source.column,
                updates: &noop,
            }];
            let plan = image(&source.expected)
                .plan_retained_indexed_long_updates(source.table, &groups, &mut budget(), &|| false)
                .unwrap();
            same(plan.after_bytes(), &source.expected);
            assert_eq!(plan.changed_rows(), 0);
        }
    }
}

#[test]
fn duplicate_late_mismatch_unindexed_source_and_cancellation_never_return_a_prefix() {
    let case = raw::source(Population::NullableDuplicate, false);
    let source = &case.creation;
    for requests in [
        vec![
            FixedScalarUpdate::new(0, Value::Null, Value::Int32(7)),
            FixedScalarUpdate::new(0, Value::Null, Value::Null),
        ],
        vec![
            FixedScalarUpdate::new(0, Value::Null, Value::Int32(7)),
            FixedScalarUpdate::new(5, Value::Int32(999), Value::Null),
        ],
        vec![
            FixedScalarUpdate::new(0, Value::Null, Value::Int32(7)),
            FixedScalarUpdate::new(999, Value::Null, Value::Int32(1)),
        ],
    ] {
        let groups = [FittedColumnUpdates::Fixed {
            column: source.column,
            updates: &requests,
        }];
        assert!(
            image(&source.expected)
                .plan_retained_indexed_long_updates(source.table, &groups, &mut budget(), &|| false)
                .is_err()
        );
    }
    let requests = updates(&case);
    let groups = [FittedColumnUpdates::Fixed {
        column: source.column,
        updates: &requests,
    }];
    assert!(
        image(&source.before)
            .plan_retained_indexed_long_updates(source.table, &groups, &mut budget(), &|| false)
            .is_err()
    );
    assert!(
        image(&source.expected)
            .plan_retained_indexed_long_updates(source.table, &groups, &mut budget(), &|| true)
            .is_err()
    );
    assert!(
        image(&source.expected)
            .plan_retained_indexed_long_updates(
                source.table,
                &groups,
                &mut ResourceBudget::new(0, 0, 0),
                &|| false
            )
            .is_err()
    );
    assert!(
        image(&source.expected)
            .plan_retained_long_column_batch_updates(source.table, &groups, &mut budget(), &|| {
                false
            })
            .is_err(),
        "old unindexed25 remains closed"
    );
}

#[test]
fn preexisting_unique_and_required_constraints_allow_valid_effects_and_refuse_violations() {
    use crate::{EditError, IndexConstraintViolation, InspectError, inspect_table_indexes};
    for descending in [false, true] {
        for required in [false, true] {
            let case = raw::unique(descending, required);
            let source = &case.creation;
            let created = image(&source.before)
                .prepare_retained_long_index_creation(
                    source.table,
                    &SecondaryIndexCreate::new(source.index_name, source.column, descending),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            let mut ordinary = source.expected.to_vec();
            let flag = source.target_definition as usize * 4096 + 170;
            assert_eq!(ordinary[flag], if required { 0x89 } else { 0x81 });
            ordinary[flag] = 0x80;
            same(created.candidate_bytes(), &ordinary);
            let inspected = inspect_table_indexes(
                &mut Cursor::new(&source.expected),
                CatalogScope::new(32611),
                source.table,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let selected = inspected
                .indexes()
                .iter()
                .find(|index| index.name() == source.index_name)
                .unwrap();
            let constraints = selected.constraints().unwrap();
            assert!(constraints.unique());
            assert_eq!(constraints.required(), required);
            let requests = updates(&case);
            let groups = [FittedColumnUpdates::Fixed {
                column: source.column,
                updates: &requests,
            }];
            let allowed = image(&source.expected)
                .plan_retained_indexed_long_updates(source.table, &groups, &mut budget(), &|| false)
                .unwrap();
            same(allowed.after_bytes(), &case.expected);
            assert_eq!(allowed.changed_rows(), 2);
            let duplicate = [
                FixedScalarUpdate::new(2, Value::Int32(3), Value::Int32(5)),
                FixedScalarUpdate::new(0, Value::Int32(1), Value::Int32(2)),
            ];
            let groups = [FittedColumnUpdates::Fixed {
                column: source.column,
                updates: &duplicate,
            }];
            let refused = image(&source.expected).plan_retained_indexed_long_updates(
                source.table,
                &groups,
                &mut budget(),
                &|| false,
            );
            assert!(matches!(
                refused,
                Err(EditError::Inspect(InspectError::IndexConstraint(
                    IndexConstraintViolation::DuplicateKey
                )))
            ));
            if required {
                let null = [
                    FixedScalarUpdate::new(2, Value::Int32(3), Value::Int32(5)),
                    FixedScalarUpdate::new(0, Value::Int32(1), Value::Null),
                ];
                let groups = [FittedColumnUpdates::Fixed {
                    column: source.column,
                    updates: &null,
                }];
                let refused = image(&source.expected).plan_retained_indexed_long_updates(
                    source.table,
                    &groups,
                    &mut budget(),
                    &|| false,
                );
                assert!(matches!(
                    refused,
                    Err(EditError::Inspect(InspectError::IndexConstraint(
                        IndexConstraintViolation::NullKey
                    )))
                ));
            }
        }
    }
}

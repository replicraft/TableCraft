//! Independent original rows, complete fixed-field effects and atomic refusals.
mod scale;
use crate::{EditError, FittedColumnUpdates, FixedScalarUpdate, NativeImage};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    field_changes, long_column_batch as raw,
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 1_000_000, 500_000_000)
}

fn image(before: &[u8]) -> NativeImage {
    NativeImage::capture_retained(
        &mut Cursor::new(before),
        CatalogScope::new(32510),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn updates(case: &raw::Source) -> Vec<FixedScalarUpdate> {
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

fn same(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    assert_eq!(
        actual.iter().zip(expected).position(|(a, b)| a != b),
        None,
        "first differing complete image byte"
    );
}

#[test]
fn atomic_long_batches_match_protected_null_forwarded_and_cross_page_images() {
    for case in [
        raw::two_rows(),
        raw::three_rows(),
        raw::forwarded(),
        raw::cross_page(),
        raw::materializing(),
    ] {
        let updates = updates(&case);
        let groups = [FittedColumnUpdates::Fixed {
            column: case.column,
            updates: &updates,
        }];
        let plan = image(&case.before)
            .plan_retained_long_column_batch_updates(case.table, &groups, &mut budget(), &|| false)
            .unwrap();
        same(plan.before_bytes(), &case.before);
        same(plan.after_bytes(), &case.expected);
        assert_eq!(
            plan.changed_rows(),
            case.cells
                .iter()
                .filter(|cell| cell.expected != cell.value)
                .count()
        );
        assert!(
            image(&case.before)
                .plan_retained_row_updates(case.table, &groups, &mut budget(), &|| false)
                .is_err(),
            "one-cell native17 must remain closed"
        );
    }
    let case = raw::two_rows();
    let unchanged = case
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
        column: case.column,
        updates: &unchanged,
    }];
    let plan = image(&case.before)
        .plan_retained_long_column_batch_updates(case.table, &groups, &mut budget(), &|| false)
        .unwrap();
    same(plan.after_bytes(), &case.before);
    assert_eq!(plan.changed_rows(), 0);
}

#[test]
fn duplicate_late_mismatch_unknown_generated_and_small_requests_produce_no_plan() {
    let case = raw::two_rows();
    for updates in [
        Vec::new(),
        vec![FixedScalarUpdate::new(
            0,
            Value::Int32(17),
            Value::Int32(22),
        )],
        vec![
            FixedScalarUpdate::new(0, Value::Int32(17), Value::Int32(22)),
            FixedScalarUpdate::new(0, Value::Int32(17), Value::Null),
        ],
        vec![
            FixedScalarUpdate::new(0, Value::Int32(17), Value::Int32(22)),
            FixedScalarUpdate::new(1, Value::Int32(999), Value::Int32(5)),
        ],
        vec![
            FixedScalarUpdate::new(0, Value::Int32(17), Value::Int32(22)),
            FixedScalarUpdate::new(999, Value::Null, Value::Int32(5)),
        ],
        vec![
            FixedScalarUpdate::new(0, Value::Int32(17), Value::Int32(22)),
            FixedScalarUpdate::new(1, Value::Null, Value::Int64(5)),
        ],
    ] {
        let before = case.before.to_vec();
        let groups = [FittedColumnUpdates::Fixed {
            column: case.column,
            updates: &updates,
        }];
        assert!(
            image(&before)
                .plan_retained_long_column_batch_updates(
                    case.table,
                    &groups,
                    &mut budget(),
                    &|| false
                )
                .is_err()
        );
        same(&before, &case.before);
    }
    let updates = updates(&case);
    let generated = [
        FixedScalarUpdate::new(0, Value::Int32(1), Value::Int32(3)),
        FixedScalarUpdate::new(1, Value::Int32(2), Value::Int32(4)),
    ];
    for (column, requests) in [
        ("ID", generated.as_slice()),
        ("Missing", updates.as_slice()),
    ] {
        let groups = [FittedColumnUpdates::Fixed {
            column,
            updates: requests,
        }];
        assert!(
            image(&case.before)
                .plan_retained_long_column_batch_updates(
                    case.table,
                    &groups,
                    &mut budget(),
                    &|| false,
                )
                .is_err()
        );
    }
    let groups = [
        FittedColumnUpdates::Fixed {
            column: "Score",
            updates: &updates,
        },
        FittedColumnUpdates::Fixed {
            column: "Score",
            updates: &updates,
        },
    ];
    assert!(
        image(&case.before)
            .plan_retained_long_column_batch_updates(case.table, &groups, &mut budget(), &|| false)
            .is_err()
    );
}

#[test]
fn cancelled_or_unpaid_batch_never_publishes_a_partial_candidate() {
    let case = raw::three_rows();
    let updates = updates(&case);
    let groups = [FittedColumnUpdates::Fixed {
        column: case.column,
        updates: &updates,
    }];
    let calls = Cell::new(0_usize);
    let completed = image(&case.before)
        .plan_retained_long_column_batch_updates(case.table, &groups, &mut budget(), &|| {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap();
    same(completed.after_bytes(), &case.expected);
    let total = calls.get();
    assert!(total > 1);
    let calls = Cell::new(0_usize);
    assert!(matches!(
        image(&case.before).plan_retained_long_column_batch_updates(
            case.table,
            &groups,
            &mut budget(),
            &|| {
                calls.set(calls.get() + 1);
                calls.get() >= total
            }
        ),
        Err(EditError::Inspect(crate::InspectError::Cancelled))
    ));
    assert!(matches!(
        image(&case.before).plan_retained_long_column_batch_updates(
            case.table,
            &groups,
            &mut ResourceBudget::new(0, 0, 0),
            &|| false
        ),
        Err(EditError::Inspect(crate::InspectError::Limit(_)))
    ));
}

#[test]
fn independent_batch_proof_rejects_changed_scope_and_protected_bytes() {
    let case = raw::two_rows();
    let updates = updates(&case);
    let groups = [FittedColumnUpdates::Fixed {
        column: case.column,
        updates: &updates,
    }];
    let original = image(&case.before);
    super::proof::prove(
        &original,
        case.table,
        &groups,
        &case.expected,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let wrong_cell = field_changes::update_score(&case.expected, case.definition, 0, Some(23));
    assert!(
        super::proof::prove(
            &original,
            case.table,
            &groups,
            &wrong_cell,
            &mut budget(),
            &|| false
        )
        .is_err()
    );
    for offset in [3 * 4096 + 3000, case.definition as usize * 4096 + 3000] {
        let mut forged = case.expected.to_vec();
        forged[offset] ^= 1;
        assert!(
            super::proof::prove(
                &original,
                case.table,
                &groups,
                &forged,
                &mut budget(),
                &|| false
            )
            .is_err()
        );
    }
    same(original.as_bytes(), &case.before);
}

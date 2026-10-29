//! Adversarial request, allocation and source-authority integration controls.
use super::{adversarial_model as extra, budget, model, qualify, qualify_case};
use crate::{
    EditError, FixedColumnUpdates, FixedScalarUpdate, IndexCheckStatus, InspectError, NativeImage,
    RuleCheckStatus, WriteBlocker, check_table, check_table_constraints, read_table,
};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;

fn capture(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

#[test]
fn qualify_adversarial_nullable_batch_sources_before_preview() {
    for code in [5, 6] {
        for descending in [false, true] {
            let case = extra::duplicate_family(code, descending);
            qualify(&case.before, &case.original, [700, 800]);
            qualify(&case.expected, &case.rows, [700, 801]);
            assert_eq!(case.before.len(), case.expected.len());
        }
        for (bytes, name, expected_rows) in [
            (extra::two_tables(code), "Other", 3),
            (extra::table_alias(code), "Alias", 500),
        ] {
            let image = capture(&bytes);
            let checked = check_table(
                &mut image.reader(),
                CatalogScope::new(1),
                name,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(checked.rows(), expected_rows);
            assert!(
                checked
                    .indexes()
                    .iter()
                    .all(|index| index.status() == IndexCheckStatus::Checked)
            );
            let constraints = check_table_constraints(
                &mut image.reader(),
                CatalogScope::new(1),
                name,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(constraints.rows(), expected_rows);
            assert!(constraints.fields().iter().all(|field| {
                field.required() != RuleCheckStatus::Unsupported
                    && field.empty_text() != RuleCheckStatus::Unsupported
                    && field.validation() != RuleCheckStatus::Unsupported
            }));
            assert_ne!(constraints.table_validation(), RuleCheckStatus::Unsupported);
            assert!(constraints.relationships().is_empty());
            assert_eq!(
                read_table(
                    &mut image.reader(),
                    CatalogScope::new(1),
                    name,
                    &mut budget(),
                    &|| false,
                )
                .unwrap()
                .rows()
                .len(),
                expected_rows
            );
            assert_eq!(image.as_bytes(), bytes);
        }
    }
}

#[test]
fn physical_scheduling_is_independent_of_column_and_row_request_order() {
    for code in [5, 6] {
        for descending in [false, true] {
            let case = model::source(code, model::Shape::Mixed, descending, false, true);
            qualify_case(&case);
            let mut left = extra::assignments(&case, false);
            let mut right = extra::assignments(&case, true);
            let first = capture(&case.before)
                .preview_ace16_split_batch_updates(
                    &[
                        FixedColumnUpdates::new("Items", "Group", &left),
                        FixedColumnUpdates::new("Items", "Score", &right),
                    ],
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(first.candidate_bytes(), case.expected);
            assert_eq!(first.changed_rows(), 500);
            left.reverse();
            right.reverse();
            let reversed = capture(&case.before)
                .preview_ace16_split_batch_updates(
                    &[
                        FixedColumnUpdates::new("items", "score", &right),
                        FixedColumnUpdates::new("ITEMS", "group", &left),
                    ],
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(reversed.candidate_bytes(), case.expected);
            assert_eq!(reversed.changed_rows(), 500);
        }
    }
}

#[test]
fn duplicate_new_eligible_families_increment_history_once_across_aliases() {
    for code in [5, 6] {
        for descending in [false, true] {
            let case = extra::duplicate_family(code, descending);
            qualify(&case.before, &case.original, [700, 800]);
            qualify(&case.expected, &case.rows, [700, 801]);
            let left = (0..500)
                .map(|row| FixedScalarUpdate::new(row, Value::Null, Value::Int32(7)))
                .collect::<Vec<_>>();
            let right = (0..500)
                .map(|row| FixedScalarUpdate::new(row, Value::Null, Value::Int32(9)))
                .collect::<Vec<_>>();
            let candidate = capture(&case.before)
                .preview_ace16_split_batch_updates(
                    &[
                        FixedColumnUpdates::new("Items", "Score", &right),
                        FixedColumnUpdates::new("Items", "Group", &left),
                    ],
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(candidate.candidate_bytes(), case.expected);
            assert_eq!(candidate.changed_rows(), 500);
        }
    }
}

#[test]
fn stale_duplicate_wrong_carrier_and_missing_ordinals_refuse_atomically() {
    let case = model::source(6, model::Shape::Mixed, false, false, false);
    qualify_case(&case);
    for (updates, expected) in [
        (
            vec![FixedScalarUpdate::new(0, Value::Int32(1), Value::Int32(2))],
            EditError::Conflict,
        ),
        (
            vec![FixedScalarUpdate::new(500, Value::Null, Value::Int32(2))],
            EditError::Conflict,
        ),
        (
            vec![
                FixedScalarUpdate::new(0, Value::Null, Value::Int32(2)),
                FixedScalarUpdate::new(0, Value::Null, Value::Int32(3)),
            ],
            EditError::Conflict,
        ),
        (
            vec![FixedScalarUpdate::new(
                0,
                Value::Null,
                Value::Text("wrong".into()),
            )],
            EditError::Unsupported(WriteBlocker::Column),
        ),
    ] {
        assert_eq!(
            capture(&case.before)
                .preview_ace16_split_batch_updates(
                    &[FixedColumnUpdates::new("Items", "Group", &updates)],
                    &mut budget(),
                    &|| false,
                )
                .err(),
            Some(expected)
        );
    }
    let updates = [FixedScalarUpdate::new(0, Value::Null, Value::Int32(2))];
    assert_eq!(
        capture(&case.before)
            .preview_ace16_split_batch_updates(
                &[
                    FixedColumnUpdates::new("Items", "Group", &updates),
                    FixedColumnUpdates::new("items", "gRoUp", &updates)
                ],
                &mut budget(),
                &|| false,
            )
            .err(),
        Some(EditError::Conflict)
    );
}

#[test]
fn late_table_stale_values_and_definition_aliases_cannot_acquire_append_authority() {
    for code in [5, 6] {
        let bytes = extra::two_tables(code);
        let left = [FixedScalarUpdate::new(0, Value::Null, Value::Int32(7))];
        let other = [FixedScalarUpdate::new(
            0,
            Value::Int32(-1),
            Value::Int32(101),
        )];
        assert_eq!(
            capture(&bytes)
                .preview_ace16_split_batch_updates(
                    &[
                        FixedColumnUpdates::new("Items", "Group", &left),
                        FixedColumnUpdates::new("Other", "Score", &other)
                    ],
                    &mut budget(),
                    &|| false,
                )
                .err(),
            Some(EditError::Conflict)
        );

        // A separately named native catalog ID aliases the selected physical TD.
        // Source admission must precede this deliberately stale assignment.
        let bytes = extra::table_alias(code);
        let stale = [FixedScalarUpdate::new(0, Value::Int32(-1), Value::Int32(7))];
        assert_eq!(
            capture(&bytes)
                .preview_ace16_split_batch_updates(
                    &[FixedColumnUpdates::new("Items", "Group", &stale)],
                    &mut budget(),
                    &|| false,
                )
                .err(),
            Some(EditError::Unsupported(WriteBlocker::Table))
        );
    }
}

#[test]
fn source_data_index_page_overlap_refuses_before_stale_field_staging() {
    for code in [5, 6] {
        let case = model::source(code, model::Shape::Mixed, false, false, false);
        qualify_case(&case);
        let mut bytes = case.before;
        let owned = model::span(&bytes, 11 << 8);
        // The literal ordinary-owned map claims an actual INDEX root as DATA.
        // It must fail the original source check before the stale field check.
        bytes[owned.start + 6] |= 0x20;
        let image = capture(&bytes);
        assert_eq!(
            check_table(
                &mut image.reader(),
                CatalogScope::new(1),
                "Items",
                &mut budget(),
                &|| false,
            )
            .err(),
            Some(InspectError::Malformed)
        );
        let stale = [FixedScalarUpdate::new(0, Value::Int32(-1), Value::Int32(7))];
        assert_eq!(
            image
                .preview_ace16_split_batch_updates(
                    &[FixedColumnUpdates::new("Items", "Group", &stale)],
                    &mut budget(),
                    &|| false,
                )
                .err(),
            Some(EditError::Inspect(InspectError::Malformed))
        );
    }
}

#[test]
fn emptying_a_retained_ignored_node_refuses_without_topology_retirement() {
    for code in [5, 6] {
        let case = model::source(code, model::Shape::Branch, false, false, false);
        qualify_case(&case);
        let left = case
            .original
            .iter()
            .enumerate()
            .map(|(row, original)| {
                FixedScalarUpdate::new(
                    row,
                    original.left.map_or(Value::Null, Value::Int32),
                    Value::Null,
                )
            })
            .collect::<Vec<_>>();
        let right = (0..case.original.len())
            .map(|row| FixedScalarUpdate::new(row, Value::Null, Value::Null))
            .collect::<Vec<_>>();
        assert_eq!(
            capture(&case.before)
                .preview_ace16_split_batch_updates(
                    &[
                        FixedColumnUpdates::new("Items", "Group", &left),
                        FixedColumnUpdates::new("Items", "Score", &right),
                    ],
                    &mut budget(),
                    &|| false,
                )
                .err(),
            Some(EditError::Unsupported(WriteBlocker::IndexedColumn))
        );
    }
}

#[test]
fn split_batches_pay_all_pools_and_cancel_at_the_final_callback() {
    let case = model::source(6, model::Shape::Mixed, true, false, false);
    let left = extra::assignments(&case, false);
    let right = extra::assignments(&case, true);
    let groups = [
        FixedColumnUpdates::new("Items", "Group", &left),
        FixedColumnUpdates::new("Items", "Score", &right),
    ];
    let mut measured = budget();
    let polls = Cell::new(0);
    let healthy = capture(&case.before)
        .preview_ace16_split_batch_updates(&groups, &mut measured, &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap();
    assert_eq!(healthy.candidate_bytes(), case.expected);
    let terminal = polls.get();
    assert!(terminal > 1);
    for resource in [Resource::Bytes, Resource::Rows, Resource::Steps] {
        let consumed = budget().remaining(resource) - measured.remaining(resource);
        assert!(consumed > 0);
        let mut limits = [512 * 1024 * 1024, 100_000, 500_000_000];
        let position = match resource {
            Resource::Bytes => 0,
            Resource::Rows => 1,
            Resource::Steps => 2,
        };
        limits[position] = consumed - 1;
        let mut limited = ResourceBudget::new(limits[0], limits[1], limits[2]);
        assert!(
            matches!(capture(&case.before).preview_ace16_split_batch_updates(&groups, &mut limited, &|| false),
            Err(EditError::Inspect(InspectError::Limit(error))) if error.resource == resource)
        );
    }
    polls.set(0);
    assert_eq!(
        capture(&case.before)
            .preview_ace16_split_batch_updates(&groups, &mut budget(), &|| {
                polls.set(polls.get() + 1);
                polls.get() == terminal
            })
            .err(),
        Some(EditError::Inspect(InspectError::Cancelled))
    );
}

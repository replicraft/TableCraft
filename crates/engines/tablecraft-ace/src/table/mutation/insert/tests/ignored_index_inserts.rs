//! Complete images for omitted secondary keys and shared physical allocation.
use super::{model as original, multiple_indexes::model as frames};
use crate::{EditError, IndexConstraintViolation, InspectError, RowInsert};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access as raw;

pub(in crate::table::mutation::insert) mod model;
use model::{Case, PAGE, Shape};

fn row(id: i32, group: Option<i32>) -> frames::Row {
    frames::Row {
        id,
        group,
        binary: Some(vec![]),
    }
}

fn run(case: &Case, rows: &[frames::Row]) -> Vec<u8> {
    original::checked(&case.source.before, case.source.rows.len(), 4);
    let expected = model::expected(case, rows);
    original::checked(&expected, case.source.rows.len() + rows.len(), 4);
    let requests = rows
        .iter()
        .map(|row| RowInsert::new(row.values()))
        .collect::<Vec<_>>();
    let candidate = original::capture(&case.source.before)
        .preview_ace16_insert_rows("Items", &requests, &mut original::budget(), &|| false)
        .unwrap();
    assert_eq!(candidate.candidate_bytes(), expected);
    assert_eq!(candidate.changed_rows(), rows.len());
    original::checked(
        candidate.candidate_bytes(),
        case.source.rows.len() + rows.len(),
        4,
    );
    expected
}

fn secondary_unchanged(case: &Case, after: &[u8]) {
    for page in &case.index_pages[1] {
        let start = *page as usize * PAGE;
        assert_eq!(
            &after[start..start + PAGE],
            &case.source.before[start..start + PAGE]
        );
    }
    let counter = 10 * PAGE + if case.primary_last { 67 } else { 79 };
    assert_eq!(
        &after[counter..counter + 4],
        &case.source.before[counter..counter + 4]
    );
    assert_eq!(
        &after[15 * PAGE..16 * PAGE],
        &case.source.before[15 * PAGE..16 * PAGE]
    );
}

#[test]
fn omitted_nulls_and_duplicate_families_have_one_nonnull_historical_delta() {
    for code in [5, 6] {
        for descending in [false, true] {
            for primary_last in [false, true] {
                let case = model::source(code, Shape::Mixed, descending, false, primary_last);
                let after = run(
                    &case,
                    &[
                        row(4, Some(10)),
                        row(5, Some(20)),
                        row(6, Some(20)),
                        row(7, None),
                    ],
                );
                let offset = 10 * PAGE + if primary_last { 67 } else { 79 };
                assert_eq!(&after[offset..offset + 4], &30_u32.to_le_bytes());
            }
        }
    }
}

#[test]
fn unique_secondary_omits_multiple_nulls_but_rejects_real_duplicate_keys() {
    for code in [5, 6] {
        for descending in [false, true] {
            let case = model::source(code, Shape::Mixed, descending, true, true);
            run(&case, &[row(4, None), row(5, None), row(6, Some(30))]);
            for rows in [
                vec![row(4, Some(10))],
                vec![row(4, Some(30)), row(5, Some(30))],
            ] {
                let requests = rows
                    .iter()
                    .map(|row| RowInsert::new(row.values()))
                    .collect::<Vec<_>>();
                assert_eq!(
                    original::capture(&case.source.before)
                        .preview_ace16_insert_rows(
                            "Items",
                            &requests,
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

#[test]
fn empty_secondary_all_null_and_first_nonnull_use_the_same_original() {
    for code in [5, 6] {
        for descending in [false, true] {
            for unique in [false, true] {
                let case = model::source(code, Shape::Empty, descending, unique, false);
                let after = run(&case, &[row(4, None), row(5, None)]);
                secondary_unchanged(&case, &after);
                let after = run(&case, &[row(4, Some(-10)), row(5, None)]);
                assert_ne!(
                    &after[14 * PAGE..15 * PAGE],
                    &case.source.before[14 * PAGE..15 * PAGE]
                );
                assert_eq!(after.len(), case.source.before.len());
            }
        }
    }
}

#[test]
fn all_null_contributions_preserve_existing_node_and_forwarded_logical_roots() {
    for code in [5, 6] {
        for descending in [false, true] {
            let case = model::source(code, Shape::Node, descending, true, true);
            let after = run(
                &case,
                &(201..209).map(|id| row(id, None)).collect::<Vec<_>>(),
            );
            secondary_unchanged(&case, &after);
            assert_eq!(after.len(), case.source.before.len());
            for slot in [0, 2] {
                let case = model::forward(
                    model::source(code, Shape::Mixed, descending, false, true),
                    slot,
                );
                let after = run(&case, &[row(4, None)]);
                secondary_unchanged(&case, &after);
                let hidden = case.source.before.len() - PAGE;
                assert_eq!(
                    &after[hidden..hidden + PAGE],
                    &case.source.before[hidden..hidden + PAGE]
                );
            }
        }
    }
}

#[test]
fn data_eof_precedes_both_physical_splits_when_primary_is_second() {
    for code in [5, 6] {
        for descending in [false, true] {
            let case = model::source(code, Shape::Growth, descending, true, true);
            let mut rows = (201..255).map(|id| row(id, Some(id))).collect::<Vec<_>>();
            rows.extend([row(255, None), row(256, None)]);
            let after = run(&case, &rows);
            assert_eq!(after.len(), 23 * PAGE);
            assert_eq!(after[18 * PAGE], 1);
            for (root, right) in [(14, 19_u32), (13, 21_u32)] {
                assert_eq!(after[root * PAGE], 3);
                assert_eq!(
                    &after[root * PAGE + 20..root * PAGE + 24],
                    &right.to_le_bytes()
                );
            }
            assert_eq!(
                &after[10 * PAGE + 67..10 * PAGE + 71],
                &883_u32.to_le_bytes()
            );
            assert_eq!(
                &after[10 * PAGE + 79..10 * PAGE + 83],
                &473_u32.to_le_bytes()
            );
        }
    }
}

#[test]
fn omitted_nulls_do_not_overflow_secondary_history_and_cancellation_stays_typed() {
    let mut case = model::source(6, Shape::Mixed, false, false, false);
    case.source.histories[1] = u32::MAX;
    raw::put32(&mut case.source.before, 10 * PAGE + 79, u32::MAX);
    let after = run(&case, &[row(4, None)]);
    secondary_unchanged(&case, &after);
    let request = RowInsert::new(row(4, Some(20)).values());
    assert_eq!(
        original::capture(&case.source.before)
            .preview_ace16_insert_rows("Items", &[request], &mut original::budget(), &|| false)
            .err(),
        Some(EditError::Inspect(InspectError::StructureLimit))
    );
    assert_eq!(
        original::capture(&case.source.before)
            .preview_ace16_insert_rows(
                "Items",
                &[RowInsert::new(row(4, None).values())],
                &mut original::budget(),
                &|| true
            )
            .err(),
        Some(EditError::Inspect(InspectError::Cancelled))
    );
}

#[test]
fn required_ignored_combinations_remain_refused_on_complete_nonnull_sources() {
    for code in [5, 6] {
        for ignored_primary in [false, true] {
            let case = model::required_source(code, ignored_primary);
            original::checked(&case.source.before, 3, 4);
            assert_eq!(
                original::capture(&case.source.before)
                    .preview_ace16_insert_rows(
                        "Items",
                        &[RowInsert::new(row(4, Some(40)).values())],
                        &mut original::budget(),
                        &|| false
                    )
                    .err(),
                Some(EditError::Unsupported(crate::WriteBlocker::IndexedColumn))
            );
        }
    }
}

#[test]
fn null_wire_missing_eligible_key_and_phantom_row_are_not_omission_authority() {
    for code in [5, 6] {
        for variant in 0..3 {
            let healthy = model::source(code, Shape::Mixed, false, false, false);
            original::checked(&healthy.source.before, 3, 4);
            let case = model::malformed_membership(healthy, variant);
            assert_eq!(
                crate::check_table(
                    &mut Cursor::new(&case.source.before),
                    CatalogScope::new(1),
                    "Items",
                    &mut original::budget(),
                    &|| false
                )
                .err(),
                Some(InspectError::Malformed)
            );
            assert_eq!(
                original::capture(&case.source.before)
                    .preview_ace16_insert_rows(
                        "Items",
                        &[RowInsert::new(row(4, None).values())],
                        &mut original::budget(),
                        &|| false
                    )
                    .err(),
                Some(EditError::Inspect(InspectError::Malformed))
            );
        }
    }
}

#[test]
fn ignored_membership_keeps_all_resource_pools_and_terminal_cancellation() {
    let case = model::source(6, Shape::Mixed, false, false, true);
    let rows = [row(4, None), row(5, Some(20))];
    let expected = model::expected(&case, &rows);
    original::checked(&case.source.before, 3, 4);
    original::checked(&expected, 5, 4);
    let requests = rows
        .iter()
        .map(|row| RowInsert::new(row.values()))
        .collect::<Vec<_>>();
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
        let result = original::capture(&case.source.before).preview_ace16_insert_rows(
            "Items",
            &requests,
            &mut budget,
            &|| false,
        );
        assert!(
            matches!(result, Err(EditError::Inspect(InspectError::Limit(limit))) if limit.resource == resource)
        );
    }
    let calls = Cell::new(0_usize);
    let candidate = original::capture(&case.source.before)
        .preview_ace16_insert_rows("Items", &requests, &mut original::budget(), &|| {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap();
    assert_eq!(candidate.candidate_bytes(), expected);
    assert!(calls.get() > 2);
    let terminal = calls.get() - 1;
    let current = Cell::new(0);
    assert_eq!(
        original::capture(&case.source.before)
            .preview_ace16_insert_rows("Items", &requests, &mut original::budget(), &|| {
                current.set(current.get() + 1);
                current.get() == terminal
            })
            .err(),
        Some(EditError::Inspect(InspectError::Cancelled))
    );
}

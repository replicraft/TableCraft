//! Public INSERT maintains every physical index, with independently built images.
use super::model as original;
use crate::{
    EditError, IndexConstraintViolation, InspectError, RowInsert, inspect_table_index_counter,
};
use std::{cell::Cell, io::Cursor};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access as raw;
use tablecraft_testkit::access::multiple_indexes as raw_source;

pub(in crate::table::mutation) mod model;
use tablecraft_testkit::access::multiple_indexes::{Case, PAGE, Row};

fn run(case: &Case, rows: &[Row]) -> Vec<u8> {
    original::checked(&case.before, case.rows.len(), 4);
    let expected = model::expected(case, rows);
    original::checked(&expected, case.rows.len() + rows.len(), 4);
    let requests = rows
        .iter()
        .map(|row| RowInsert::new(row.values()))
        .collect::<Vec<_>>();
    let candidate = original::capture(&case.before)
        .preview_ace16_insert_rows("Items", &requests, &mut original::budget(), &|| false)
        .unwrap();
    assert_eq!(candidate.candidate_bytes(), expected);
    assert_eq!(candidate.changed_rows(), rows.len());
    original::checked(candidate.candidate_bytes(), case.rows.len() + rows.len(), 4);
    for names in [["PrimaryID", "PrimaryAlias"], ["GroupOrder", "GroupAlias"]] {
        let mut prior = None;
        for name in names {
            let counter = inspect_table_index_counter(
                &mut Cursor::new(candidate.candidate_bytes()),
                CatalogScope::new(1),
                "Items",
                name,
                &mut original::budget(),
                &|| false,
            )
            .unwrap();
            let facts = (
                counter.physical_index(),
                counter.byte_range(),
                counter.value(),
            );
            if let Some(prior) = &prior {
                assert_eq!(&facts, prior);
            }
            prior = Some(facts);
        }
    }
    expected
}

#[test]
fn aliases_duplicate_and_null_secondary_keys_get_one_physical_historical_delta() {
    for code in [5, 6] {
        let case = raw_source::source(code, false, false);
        let rows = [(4, Some(10)), (5, Some(20)), (6, Some(20)), (7, None)]
            .into_iter()
            .map(|(id, group)| Row {
                id,
                group,
                binary: Some(vec![]),
            })
            .collect::<Vec<_>>();
        let after = run(&case, &rows);
        assert_eq!(
            &after[10 * PAGE + 67..10 * PAGE + 71],
            &21_u32.to_le_bytes()
        );
        assert_eq!(
            &after[10 * PAGE + 79..10 * PAGE + 83],
            &30_u32.to_le_bytes()
        );
        assert_eq!(after.len(), case.before.len());
        // A fresh public no-op must preserve the complete image without granting
        // the model's builder facts any authority over the new operation.
        let candidate = original::capture(&after)
            .preview_ace16_insert_rows("Items", &[], &mut original::budget(), &|| false)
            .unwrap();
        assert_eq!(candidate.candidate_bytes(), after);
    }
}

#[test]
fn unique_secondary_checks_existing_and_batch_duplicates_but_allows_null_family() {
    for code in [5, 6] {
        let case = raw_source::source(code, false, true);
        let rows = [
            Row {
                id: 4,
                group: None,
                binary: None,
            },
            Row {
                id: 5,
                group: None,
                binary: Some(vec![]),
            },
            Row {
                id: 6,
                group: Some(40),
                binary: None,
            },
        ];
        let after = run(&case, &rows);
        assert_eq!(
            &after[10 * PAGE + 79..10 * PAGE + 83],
            &30_u32.to_le_bytes()
        );
        for keys in [vec![10], vec![40, 40]] {
            let requests = keys
                .into_iter()
                .enumerate()
                .map(|(position, key)| {
                    RowInsert::new(
                        Row {
                            id: 10 + position as i32,
                            group: Some(key),
                            binary: None,
                        }
                        .values(),
                    )
                })
                .collect::<Vec<_>>();
            assert_eq!(
                original::capture(&case.before)
                    .preview_ace16_insert_rows("Items", &requests, &mut original::budget(), &|| {
                        false
                    })
                    .err(),
                Some(EditError::Inspect(InspectError::IndexConstraint(
                    IndexConstraintViolation::DuplicateKey
                )))
            );
        }
    }
}

#[test]
fn two_root_splits_share_data_first_cursor_and_global_byte_without_role_aliasing() {
    for code in [5, 6] {
        let case = raw_source::source(code, true, false);
        let rows = (201..253)
            .map(|id| Row {
                id,
                group: Some(id),
                binary: Some(vec![]),
            })
            .collect::<Vec<_>>();
        let after = run(&case, &rows);
        assert_eq!(after.len(), 23 * PAGE);
        for (root, right, left) in [(13, 19, 20), (14, 21, 22)] {
            assert_eq!(after[root * PAGE], 3);
            assert_eq!(
                &after[root * PAGE + 20..root * PAGE + 24],
                &(right as u32).to_le_bytes()
            );
            assert_eq!(
                &after[(left as usize) * PAGE + 16..(left as usize) * PAGE + 20],
                &(right as u32).to_le_bytes()
            );
            assert_eq!(
                &after[(right as usize) * PAGE + 12..(right as usize) * PAGE + 16],
                &(left as u32).to_le_bytes()
            );
        }
        assert_eq!(
            &after[10 * PAGE + 67..10 * PAGE + 71],
            &469_u32.to_le_bytes()
        );
        assert_eq!(
            &after[10 * PAGE + 79..10 * PAGE + 83],
            &881_u32.to_le_bytes()
        );
        assert_eq!(after[18 * PAGE], 1); // DATA18, not index or bitmap.
        assert_eq!(
            &after[18 * PAGE + 12..18 * PAGE + 14],
            &38_u16.to_le_bytes()
        );
        assert_eq!(
            &after[17 * PAGE + 12..17 * PAGE + 14],
            &214_u16.to_le_bytes()
        );
    }
}

#[test]
fn per_physical_counter_overflow_and_cancellation_return_no_candidate() {
    let mut case = raw_source::source(6, false, false);
    raw::put32(&mut case.before, 10 * PAGE + 79, u32::MAX);
    original::checked(&case.before, 3, 4);
    let request = RowInsert::new(
        Row {
            id: 4,
            group: Some(20),
            binary: None,
        }
        .values(),
    );
    assert_eq!(
        original::capture(&case.before)
            .preview_ace16_insert_rows("Items", &[request], &mut original::budget(), &|| false)
            .err(),
        Some(EditError::Inspect(InspectError::StructureLimit))
    );
    let case = raw_source::source(6, false, false);
    let cancelled = Cell::new(false);
    let image = original::capture(&case.before);
    cancelled.set(true);
    assert_eq!(
        image
            .preview_ace16_insert_rows(
                "Items",
                &[RowInsert::new(
                    Row {
                        id: 4,
                        group: Some(20),
                        binary: None
                    }
                    .values()
                )],
                &mut original::budget(),
                &|| cancelled.get()
            )
            .err(),
        Some(EditError::Inspect(InspectError::Cancelled))
    );
}

#[test]
fn descending_secondary_keeps_direction_and_ignored_null_insert_preserves_secondary() {
    for code in [5, 6] {
        let case = model::descending(raw_source::source(code, false, false));
        run(
            &case,
            &[
                Row {
                    id: 4,
                    group: Some(-20),
                    binary: None,
                },
                Row {
                    id: 5,
                    group: None,
                    binary: Some(vec![]),
                },
            ],
        );
        let case = model::ignored_null_source(code);
        original::checked(&case.before, 3, 4);
        let rows = [Row {
            id: 4,
            group: None,
            binary: None,
        }];
        let mut expected = model::expected(&case, &rows);
        expected[14 * PAGE..15 * PAGE].copy_from_slice(&case.before[14 * PAGE..15 * PAGE]);
        expected[10 * PAGE + 79..10 * PAGE + 83]
            .copy_from_slice(&case.before[10 * PAGE + 79..10 * PAGE + 83]);
        original::checked(&expected, 4, 4);
        let candidate = original::capture(&case.before)
            .preview_ace16_insert_rows(
                "Items",
                &[RowInsert::new(rows[0].values())],
                &mut original::budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(candidate.candidate_bytes(), expected);
        assert_eq!(
            &expected[15 * PAGE..16 * PAGE],
            &case.before[15 * PAGE..16 * PAGE]
        );
        // INSERT's complete omitted-key proof does not broaden either UPDATE API.
        assert_eq!(
            original::capture(&case.before)
                .preview_ace16_int32_updates("Items", "ID", &[], &mut original::budget(), &|| false)
                .err(),
            Some(EditError::Unsupported(
                crate::WriteBlocker::ConstraintCoverage
            ))
        );
        assert_eq!(
            original::capture(&case.before)
                .preview_ace16_slot_updates("Items", "Score", &[], &mut original::budget(), &|| {
                    false
                })
                .err(),
            Some(EditError::Unsupported(
                crate::WriteBlocker::ConstraintCoverage
            ))
        );
    }
}

#[test]
fn individually_readable_physical_indexes_cannot_share_a_mutated_tree_page() {
    for code in [5, 6] {
        let case = model::shared_root_source(code);
        let checked = original::checked(&case.before, 3, 4);
        assert_eq!(checked.schema().columns().len(), 3);
        assert_eq!(checked.indexes()[0].index().root_page(), 13);
        assert_eq!(checked.indexes()[2].index().root_page(), 13);
        assert_ne!(
            checked.indexes()[0].index().data_number(),
            checked.indexes()[2].index().data_number()
        );
        let request = RowInsert::new(
            Row {
                id: 4,
                group: Some(4),
                binary: None,
            }
            .values(),
        );
        assert_eq!(
            original::capture(&case.before)
                .preview_ace16_insert_rows("Items", &[request], &mut original::budget(), &|| false)
                .err(),
            Some(EditError::Unsupported(crate::WriteBlocker::IndexedColumn))
        );
    }
}

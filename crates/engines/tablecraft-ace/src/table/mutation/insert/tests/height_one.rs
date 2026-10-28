//! Public cardinality growth within existing height-one primary leaves.
use super::model as base;
use crate::{
    EditError, IndexConstraintViolation, InspectError, RowInsert, inspect_table_index_counter,
    read_table,
};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::{limits::Resource, values::Value};
use tablecraft_model::CatalogScope;

mod model;

const PAGE: usize = 4096;

fn checked(bytes: &[u8], rows: &[Vec<Value>], aliases: bool) {
    base::checked(bytes, rows.len(), if aliases { 2 } else { 1 });
    let table = read_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut base::budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(table.rows().len(), rows.len());
    for (position, (actual, expected)) in table.rows().iter().zip(rows).enumerate() {
        assert_eq!(actual, expected, "stored row {position}");
    }
    let mut range = None;
    for index in if aliases {
        vec!["PrimaryID", "PrimaryAlias"]
    } else {
        vec!["PrimaryID"]
    } {
        let counter = inspect_table_index_counter(
            &mut Cursor::new(bytes),
            CatalogScope::new(1),
            "Items",
            index,
            &mut base::budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(counter.value(), rows.len() as u32);
        assert_eq!(counter.physical_index(), 0);
        if let Some(original) = &range {
            assert_eq!(&counter.byte_range(), original);
        } else {
            range = Some(counter.byte_range());
        }
    }
}

fn requests(values: &[i32]) -> Vec<RowInsert> {
    values
        .iter()
        .map(|value| RowInsert::new(vec![Value::Int32(*value), Value::Binary(Vec::new())]))
        .collect()
}

#[test]
fn left_right_and_both_leaf_inserts_preserve_parent_links_maps_and_all_old_rows() {
    for code in [5, 6] {
        for aliases in [false, true] {
            let (before, original_rows) = model::source(code, aliases);
            checked(&before, &original_rows, aliases);
            for values in [vec![-227], vec![227], vec![-227, 227, 228], vec![]] {
                let mut rows = original_rows.clone();
                rows.extend(
                    values
                        .iter()
                        .map(|value| vec![Value::Int32(*value), Value::Binary(Vec::new())]),
                );
                let golden = model::expected(&before, &values);
                checked(&golden, &rows, aliases);
                let candidate = base::capture(&before)
                    .preview_ace16_insert_rows(
                        "Items",
                        &requests(&values),
                        &mut base::budget(),
                        &|| false,
                    )
                    .unwrap();
                assert_eq!(candidate.changed_rows(), values.len());
                model::assert_image(candidate.candidate_bytes(), &golden);
                checked(candidate.candidate_bytes(), &rows, aliases);
                assert_eq!(
                    &candidate.candidate_bytes()[13 * PAGE..15 * PAGE],
                    &before[13 * PAGE..15 * PAGE]
                );
                assert_eq!(
                    &candidate.candidate_bytes()[11 * PAGE..12 * PAGE],
                    &before[11 * PAGE..12 * PAGE]
                );
                assert_eq!(
                    &candidate.candidate_bytes()[PAGE..2 * PAGE],
                    &before[PAGE..2 * PAGE]
                );
                for page in [16, 17] {
                    assert_eq!(
                        &candidate.candidate_bytes()[page * PAGE + 12..page * PAGE + 24],
                        &before[page * PAGE + 12..page * PAGE + 24]
                    );
                }
                if values == [-227] {
                    assert_eq!(
                        &candidate.candidate_bytes()[16 * PAGE..17 * PAGE],
                        &before[16 * PAGE..17 * PAGE]
                    );
                }
                if values == [227] {
                    assert_eq!(
                        &candidate.candidate_bytes()[17 * PAGE..18 * PAGE],
                        &before[17 * PAGE..18 * PAGE]
                    );
                }
            }
        }
    }
}

#[test]
fn duplicate_original_boundary_refuses_and_two_overflowing_leaves_are_maintained() {
    for code in [5, 6] {
        let (before, rows) = model::source(code, true);
        checked(&before, &rows, true);
        let image = base::capture(&before);
        let duplicate = image
            .preview_ace16_insert_rows("Items", &requests(&[-1]), &mut base::budget(), &|| false)
            .err()
            .unwrap();
        assert_eq!(
            duplicate,
            EditError::Inspect(InspectError::IndexConstraint(
                IndexConstraintViolation::DuplicateKey
            ))
        );
        // Adjacent small negatives repair to a three-byte key prefix and fit.
        // Spread the new values across the negative domain so the retained -1
        // bound limits their common prefix to the marker byte instead.
        let adjacent = (-526..-226).collect::<Vec<_>>();
        assert_eq!(model::minimum_leaf_bytes(&before, 17, &adjacent), 3159);
        let overflow = (0..300)
            .map(|position| i32::MIN + position * 65536)
            .collect::<Vec<_>>();
        assert_eq!(model::minimum_leaf_bytes(&before, 17, &overflow), 4209);
        assert!(model::minimum_leaf_bytes(&before, 17, &overflow) > 3616);
        let peer_overflow = (1..=300)
            .map(|position| position * 65536)
            .collect::<Vec<_>>();
        assert_eq!(model::minimum_leaf_bytes(&before, 16, &peer_overflow), 4209);
        let overflow = overflow
            .into_iter()
            .chain(peer_overflow)
            .collect::<Vec<_>>();
        let candidate = base::capture(&before)
            .preview_ace16_insert_rows("Items", &requests(&overflow), &mut base::budget(), &|| {
                false
            })
            .unwrap();
        let mut expected_rows = rows.clone();
        expected_rows.extend(
            overflow
                .iter()
                .map(|value| vec![Value::Int32(*value), Value::Binary(Vec::new())]),
        );
        assert_eq!(candidate.changed_rows(), overflow.len());
        checked(candidate.candidate_bytes(), &expected_rows, true);
        checked(&before, &rows, true);
    }
}

#[test]
fn height_one_failure_pools_and_late_cancel_keep_source_retryable() {
    let (before, rows) = model::source(6, true);
    checked(&before, &rows, true);
    let request = requests(&[-227, 227]);
    for pool in [Resource::Bytes, Resource::Rows, Resource::Steps] {
        let mut budget = base::budget();
        budget.charge(pool, budget.remaining(pool)).unwrap();
        assert!(
            base::capture(&before)
                .preview_ace16_insert_rows("Items", &request, &mut budget, &|| false)
                .is_err()
        );
    }
    let calls = Cell::new(0);
    assert!(
        base::capture(&before)
            .preview_ace16_insert_rows("Items", &request, &mut base::budget(), &|| {
                calls.set(calls.get() + 1);
                calls.get() > 400
            })
            .is_err()
    );
    let golden = model::expected(&before, &[-227, 227]);
    let candidate = base::capture(&before)
        .preview_ace16_insert_rows("Items", &request, &mut base::budget(), &|| false)
        .unwrap();
    model::assert_image(candidate.candidate_bytes(), &golden);
    checked(&before, &rows, true);
}

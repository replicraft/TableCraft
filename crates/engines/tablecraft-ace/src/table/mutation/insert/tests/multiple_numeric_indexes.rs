//! Public two-owner INSERT with independent narrow and eight-byte numeric wires.
mod model;

use super::model as original;
use crate::{RowInsert, inspect_table_index_counter};
use model::{Case, Kind, Row};
use std::io::Cursor;
use tablecraft_model::CatalogScope;

const PAGE: usize = 4096;

fn run(case: &Case, requests: &[Row]) -> Vec<u8> {
    original::checked(&case.before, case.rows.len(), 4);
    model::assert_typed(&case.before, &case.rows, case.kind);
    // Complete independent output is fixed before invoking the preview.
    let expected = model::expected(case, requests);
    original::checked(&expected, case.rows.len() + requests.len(), 4);
    let input = requests
        .iter()
        .map(|row| RowInsert::new(row.values(case.kind)))
        .collect::<Vec<_>>();
    let actual = original::capture(&case.before)
        .preview_ace16_insert_rows("Items", &input, &mut original::budget(), &|| false)
        .unwrap();
    let difference = actual
        .candidate_bytes()
        .iter()
        .zip(&expected)
        .position(|(left, right)| left != right);
    assert_eq!(actual.candidate_bytes().len(), expected.len());
    assert!(
        difference.is_none(),
        "first byte difference at {difference:?}"
    );
    original::checked(
        actual.candidate_bytes(),
        case.rows.len() + requests.len(),
        4,
    );
    let mut expected_rows = case.rows.clone();
    expected_rows.extend(requests.iter().cloned().map(|row| (row, 0)));
    model::assert_typed(actual.candidate_bytes(), &expected_rows, case.kind);
    for name in ["GroupOrder", "GroupAlias"] {
        let history = inspect_table_index_counter(
            &mut Cursor::new(actual.candidate_bytes()),
            CatalogScope::new(1),
            "Items",
            name,
            &mut original::budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(history.physical_index(), 1);
        let expected_history =
            u32::from_le_bytes(expected[10 * PAGE + 79..10 * PAGE + 83].try_into().unwrap());
        assert_eq!(history.value(), expected_history);
    }
    expected
}

#[test]
fn byte_and_int16_extremes_nullable_duplicates_and_directions_have_exact_images() {
    for kind in [Kind::Byte, Kind::Int16] {
        for code in [5, 6] {
            for descending in [false, true] {
                let case = model::source(code, kind, descending, false);
                let minimum = kind.bounds().0;
                let requests = [Some(minimum), Some(1), Some(1), None]
                    .into_iter()
                    .enumerate()
                    .map(|(position, group)| Row {
                        id: 4 + position as i32,
                        group,
                    })
                    .collect::<Vec<_>>();
                let expected = run(&case, &requests);
                assert_eq!(
                    &expected[10 * PAGE + 79..10 * PAGE + 83],
                    &30_u32.to_le_bytes()
                );
                assert_eq!(expected.len(), case.before.len());
            }
        }
    }
}

#[test]
fn int64_and_currency_extremes_nulls_and_distinct_history_have_exact_images() {
    for kind in [Kind::Int64, Kind::Currency] {
        for code in [5, 6] {
            for descending in [false, true] {
                let case = model::source(code, kind, descending, false);
                let requests = [Some(i64::MIN), Some(1), Some(1), None]
                    .into_iter()
                    .enumerate()
                    .map(|(position, group)| Row {
                        id: 4 + position as i32,
                        group,
                    })
                    .collect::<Vec<_>>();
                let expected = run(&case, &requests);
                assert_eq!(
                    &expected[10 * PAGE + 79..10 * PAGE + 83],
                    &30_u32.to_le_bytes()
                );
            }
        }
    }
}

#[test]
fn eight_byte_secondary_growth_uses_nullable_leaf_lengths_and_seventeen_byte_bound() {
    for kind in [Kind::Int64, Kind::Currency] {
        for code in [5, 6] {
            for descending in [false, true] {
                let case = model::source(code, kind, descending, true);
                assert_eq!(
                    &case.before[14 * PAGE + 24..14 * PAGE + 26],
                    &1_u16.to_le_bytes()
                );
                assert_eq!(
                    &case.before[14 * PAGE + 2..14 * PAGE + 4],
                    &15_u16.to_le_bytes()
                );
                let requests = [None, Some(0), Some(i64::MAX)]
                    .into_iter()
                    .enumerate()
                    .map(|(position, group)| Row {
                        id: 301 + position as i32,
                        group,
                    })
                    .collect::<Vec<_>>();
                let expected = run(&case, &requests);
                assert_eq!(expected.len(), 20 * PAGE);
                assert_eq!(expected[13 * PAGE], 4); // Primary leaf still fits.
                assert_eq!(expected[14 * PAGE], 3);
                assert_eq!(
                    &expected[14 * PAGE + 2..14 * PAGE + 4],
                    &3599_u16.to_le_bytes()
                );
                assert_eq!(
                    &expected[14 * PAGE + 20..14 * PAGE + 24],
                    &18_u32.to_le_bytes()
                );
                assert_eq!(
                    &expected[10 * PAGE + 79..10 * PAGE + 83],
                    &631_u32.to_le_bytes()
                );
            }
        }
    }
}

#[test]
fn required_primary_in_physical_slot_one_retains_actual_owner_counters() {
    for code in [5, 6] {
        let case = model::source(code, Kind::Int64, false, false);
        let requests = [
            Row {
                id: 4,
                group: Some(1),
            },
            Row {
                id: 5,
                group: Some(1),
            },
            Row { id: 6, group: None },
        ];
        let mut before = case.before.clone();
        let mut expected = model::expected(&case, &requests);
        model::transpose_physical_owners(&mut before);
        model::transpose_physical_owners(&mut expected);
        original::checked(&before, 3, 4);
        original::checked(&expected, 6, 4);
        let input = requests
            .iter()
            .map(|row| RowInsert::new(row.values(case.kind)))
            .collect::<Vec<_>>();
        let candidate = original::capture(&before)
            .preview_ace16_insert_rows("Items", &input, &mut original::budget(), &|| false)
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
        original::checked(actual, 6, 4);
        for (name, physical, offset, count) in [
            ("GroupOrder", 0, 67, 30),
            ("GroupAlias", 0, 67, 30),
            ("PrimaryID", 1, 79, 20),
            ("PrimaryAlias", 1, 79, 20),
        ] {
            let history = inspect_table_index_counter(
                &mut Cursor::new(actual),
                CatalogScope::new(1),
                "Items",
                name,
                &mut original::budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(history.physical_index(), physical);
            assert_eq!(
                history.byte_range(),
                10 * PAGE + offset..10 * PAGE + offset + 4
            );
            assert_eq!(history.value(), count);
        }
    }
}

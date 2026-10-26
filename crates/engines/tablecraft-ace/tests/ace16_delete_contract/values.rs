use super::{fixture, index_fixture, model::*, numeric_fixture};
use tablecraft_ace::RowDelete;
use tablecraft_core::values::Value;

#[test]
fn ordinary_delete_candidates_change_only_live_count_and_selected_slot_flags() {
    for code in [5, 6] {
        let bytes = ace(fixture::database(), code);
        for deleted in [vec![], vec![0], vec![1], vec![2], vec![2, 0], vec![0, 1, 2]] {
            let requests = deleted
                .iter()
                .map(|&row| request(row, false))
                .collect::<Vec<_>>();
            let candidate = preview(&bytes, &requests).unwrap();
            assert_preserved(&bytes, &candidate, &deleted, None);
            let expected = (0..3)
                .filter(|row| !deleted.contains(row))
                .map(|row| {
                    vec![
                        Value::Int32(row as i32 + 1),
                        Value::Int32((row as i32 + 1) * 10),
                    ]
                })
                .collect::<Vec<_>>();
            assert_eq!(table_rows(candidate.candidate_bytes()), expected);
            if deleted.is_empty() {
                assert_eq!(candidate.candidate_bytes(), bytes);
            }
        }
    }
}

#[test]
fn root_leaf_deletes_preserve_historical_counters_aliases_and_key_order() {
    for code in [5, 6] {
        for descending in [false, true] {
            for alias in [false, true] {
                for nullable in [false, true] {
                    let bytes = ace(index_fixture::database(descending, alias, nullable), code);
                    assert_eq!(preview(&bytes, &[]).unwrap().candidate_bytes(), bytes);
                    for deleted in [vec![0], vec![1], vec![2], vec![0, 2], vec![0, 1, 2]] {
                        let requests = deleted
                            .iter()
                            .map(|&row| request(row, nullable))
                            .collect::<Vec<_>>();
                        let candidate = preview(&bytes, &requests).unwrap();
                        let after = candidate.candidate_bytes();
                        assert_preserved(&bytes, &candidate, &deleted, Some(13));
                        assert_eq!(count(after, 10 * 4096 + 67), 3);
                        let mut expected = (0..3)
                            .filter(|row| !deleted.contains(row))
                            .map(|row| row as i32 + 1)
                            .collect::<Vec<_>>();
                        if descending {
                            expected.reverse();
                        }
                        assert_eq!(index_ids(after, "ScoreKey"), expected);
                        if alias {
                            assert_eq!(index_ids(after, "ScoreAlias"), expected);
                        }
                        if deleted.len() == 3 {
                            assert_eq!(word(after, 13 * 4096 + 24), 0);
                            assert_eq!(word(after, 13 * 4096 + 2), 3616);
                            assert!(
                                after[13 * 4096 + 27..13 * 4096 + 480]
                                    .iter()
                                    .all(|byte| *byte == 0)
                            );
                            let no_op = preview(after, &[]).unwrap();
                            assert_eq!(no_op.deleted_rows(), 0);
                            assert_eq!(no_op.candidate_bytes(), after);
                        }
                    }
                }
            }
        }
    }
}

fn encoded_row(values: &[Option<i64>]) -> Vec<u8> {
    let mut bytes = (values.len() as u16).to_le_bytes().to_vec();
    for value in values {
        bytes.extend((value.unwrap_or(0) as i32).to_le_bytes());
    }
    let mask = values
        .iter()
        .enumerate()
        .fold(0_u8, |mask, (index, value)| {
            mask | (u8::from(value.is_some()) << index)
        });
    bytes.push(mask);
    bytes
}

#[test]
fn nonunique_ties_remove_only_the_requested_row_reference() {
    let rows = vec![
        vec![Some(1), Some(10)],
        vec![Some(2), Some(10)],
        vec![Some(3), None],
    ];
    for descending in [false, true] {
        let bytes = numeric_fixture::database(6, &[4, 4], &[(1, descending)], &rows, 0);
        for deleted in [vec![0], vec![1], vec![2], vec![0, 1, 2]] {
            let requests = deleted
                .iter()
                .map(|&row| RowDelete::new(row, encoded_row(&rows[row])))
                .collect::<Vec<_>>();
            let candidate = preview(&bytes, &requests).unwrap();
            assert_preserved(&bytes, &candidate, &deleted, Some(14));
            let initial = if descending { [1, 2, 3] } else { [3, 1, 2] };
            let expected = initial
                .into_iter()
                .filter(|id| !deleted.contains(&(*id as usize - 1)))
                .collect::<Vec<_>>();
            assert_eq!(index_ids(candidate.candidate_bytes(), "Compound"), expected);
            assert_eq!(
                count(candidate.candidate_bytes(), 10 * 4096 + 67),
                count(&bytes, 10 * 4096 + 67)
            );
        }
    }
}

#[test]
fn composite_nullable_keys_keep_component_directions_and_remaining_ties() {
    let rows = vec![
        vec![Some(1), Some(10), Some(2)],
        vec![Some(2), Some(10), Some(1)],
        vec![Some(3), None, Some(3)],
    ];
    for descending in [false, true] {
        let bytes = numeric_fixture::database(
            6,
            &[4, 4, 4],
            &[(1, descending), (2, !descending)],
            &rows,
            0,
        );
        for deleted in [vec![0], vec![1], vec![2], vec![0, 1, 2]] {
            let requests = deleted
                .iter()
                .map(|&row| RowDelete::new(row, encoded_row(&rows[row])))
                .collect::<Vec<_>>();
            let candidate = preview(&bytes, &requests).unwrap();
            assert_preserved(&bytes, &candidate, &deleted, Some(14));
            let mut remaining = (0..3)
                .filter(|row| !deleted.contains(row))
                .collect::<Vec<_>>();
            remaining.sort_by(|&left, &right| {
                let primary = rows[left][1].cmp(&rows[right][1]);
                let primary = if descending {
                    primary.reverse()
                } else {
                    primary
                };
                let secondary = rows[left][2].cmp(&rows[right][2]);
                let secondary = if descending {
                    secondary
                } else {
                    secondary.reverse()
                };
                primary.then(secondary).then(left.cmp(&right))
            });
            let expected = remaining
                .iter()
                .map(|row| *row as i32 + 1)
                .collect::<Vec<_>>();
            assert_eq!(index_ids(candidate.candidate_bytes(), "Compound"), expected);
        }
    }
}

#[test]
fn empty_ordinary_table_and_empty_request_produce_an_identical_candidate() {
    let mut bytes = ace(fixture::database(), 6);
    super::support::put32(&mut bytes, 10 * 4096 + 16, 0);
    bytes[12 * 4096..13 * 4096].copy_from_slice(&super::support::data_page(10, &[]));
    let candidate = preview(&bytes, &[]).unwrap();
    assert_eq!(candidate.deleted_rows(), 0);
    assert_eq!(candidate.before_bytes(), bytes);
    assert_eq!(candidate.candidate_bytes(), bytes);
    assert!(table_rows(candidate.candidate_bytes()).is_empty());
}

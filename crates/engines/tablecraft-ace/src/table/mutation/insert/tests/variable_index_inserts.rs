//! Whole-image controls authored before invoking the variable-key producer.
use super::model as original;
use crate::{EditError, RowInsert, WriteBlocker, read_table};
use std::io::Cursor;
use tablecraft_model::CatalogScope;

mod model;
use model::{Case, Family, PAGE, Row, Settings};

fn row(id: i32, group: Option<i32>, payload: Option<Vec<u8>>) -> Row {
    Row { id, group, payload }
}

fn checked(case: &Case, bytes: &[u8], added: &[Row]) {
    original::checked(bytes, case.rows.len() + added.len(), 4);
    let snapshot = read_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut original::budget(),
        &|| false,
    )
    .unwrap();
    let expected = case
        .rows
        .iter()
        .map(|(row, _)| row)
        .chain(added)
        .map(|row| row.values(case.settings.family))
        .collect::<Vec<_>>();
    assert_eq!(snapshot.rows(), expected);
}

fn run(case: &Case, added: &[Row]) -> Vec<u8> {
    checked(case, &case.before, &[]);
    let expected = model::expected(case, added);
    checked(case, &expected, added);
    let requests = added
        .iter()
        .map(|row| RowInsert::new(row.values(case.settings.family)))
        .collect::<Vec<_>>();
    let candidate = original::capture(&case.before)
        .preview_ace16_insert_rows("Items", &requests, &mut original::budget(), &|| false)
        .unwrap_or_else(|error| panic!("{:?}: {error:?}", case.settings));
    assert_eq!(candidate.changed_rows(), added.len());
    let actual = candidate.candidate_bytes();
    assert_eq!(actual.len(), expected.len());
    let difference = actual
        .iter()
        .zip(&expected)
        .position(|(actual, expected)| actual != expected);
    assert!(
        difference.is_none(),
        "first variable-key image difference: {difference:?}"
    );
    checked(case, candidate.candidate_bytes(), added);
    expected
}

#[test]
fn variable_keys_keep_null_empty_written_order_and_unselected_null_independent() {
    for code in [5, 6] {
        for family in [Family::Binary, Family::Legacy, Family::Modern] {
            for descending in [false, true] {
                for (tuple, reversed) in [(false, false), (true, false), (true, true)] {
                    for ignored in [false, true] {
                        let settings = Settings {
                            family,
                            descending,
                            tuple,
                            reversed,
                            unique: false,
                            ignored,
                        };
                        let case = model::source(
                            code,
                            settings,
                            vec![
                                row(1, None, None),
                                row(2, Some(10), Some(vec![])),
                                row(3, None, Some(b"a".to_vec())),
                            ],
                        );
                        run(
                            &case,
                            &[
                                row(10, None, Some(b"aa".to_vec())),
                                row(11, Some(20), None),
                                row(12, None, None),
                                row(13, None, Some(vec![])),
                            ],
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn complete_variable_unique_keys_allow_all_null_but_reject_partial_null_duplicates() {
    for code in [5, 6] {
        for family in [Family::Binary, Family::Legacy, Family::Modern] {
            for reversed in [false, true] {
                let settings = Settings {
                    family,
                    descending: reversed,
                    tuple: true,
                    reversed,
                    unique: true,
                    ignored: false,
                };
                let case = model::source(
                    code,
                    settings,
                    vec![row(1, None, None), row(2, None, Some(b"a".to_vec()))],
                );
                run(
                    &case,
                    &[row(10, None, None), row(11, Some(1), Some(b"a".to_vec()))],
                );
                let duplicate = RowInsert::new(row(20, None, Some(b"a".to_vec())).values(family));
                assert!(
                    original::capture(&case.before)
                        .preview_ace16_insert_rows(
                            "Items",
                            &[duplicate],
                            &mut original::budget(),
                            &|| false
                        )
                        .is_err()
                );
            }
        }
    }
}

#[test]
fn full_binary_keys_grow_a_real_root_with_585_byte_node_bounds() {
    for code in [5, 6] {
        for descending in [false, true] {
            let settings = Settings {
                family: Family::Binary,
                descending,
                tuple: false,
                reversed: false,
                unique: false,
                ignored: false,
            };
            let records = (1..=6)
                .map(|id| row(id, None, Some(vec![id as u8; 510])))
                .collect();
            let case = model::source(code, settings, records);
            let expected = run(&case, &[row(10, None, Some(vec![10; 510]))]);
            assert_eq!(expected.len(), case.before.len() + 2 * PAGE);
            assert_eq!(expected[14 * PAGE], 3);
            assert_eq!(
                u16::from_le_bytes(expected[14 * PAGE + 2..14 * PAGE + 4].try_into().unwrap()),
                3031
            );
        }
    }
}

#[test]
fn modern_digest_nonunique_keys_preserve_wire_and_unique_digest_keys_refuse() {
    for code in [5, 6] {
        for descending in [false, true] {
            let mut settings = Settings {
                family: Family::Modern,
                descending,
                tuple: false,
                reversed: false,
                unique: false,
                ignored: false,
            };
            let records = vec![row(1, None, Some(vec![b'a'; 255]))];
            let case = model::source(code, settings, records.clone());
            run(&case, &[row(10, None, Some(vec![b'a'; 255]))]);
            settings.unique = true;
            let unique = model::source(code, settings, records);
            let request = RowInsert::new(row(10, None, Some(b"a".to_vec())).values(Family::Modern));
            assert!(
                original::capture(&unique.before)
                    .preview_ace16_insert_rows(
                        "Items",
                        &[request],
                        &mut original::budget(),
                        &|| false
                    )
                    .is_err()
            );
        }
    }
}

#[test]
fn an_existing_empty_text_key_does_not_authorize_new_empty_text_without_a_declaration() {
    let settings = Settings {
        family: Family::Legacy,
        descending: false,
        tuple: false,
        reversed: false,
        unique: false,
        ignored: false,
    };
    let mut case = model::source(6, settings, vec![row(1, None, Some(vec![]))]);
    let baseline = tablecraft_testkit::access::multiple_indexes::source(6, false, false);
    case.before[4 * PAGE..5 * PAGE].copy_from_slice(&baseline.before[4 * PAGE..5 * PAGE]);
    checked(&case, &case.before, &[]);

    let request = RowInsert::new(row(10, None, Some(vec![])).values(Family::Legacy));
    let error = original::capture(&case.before)
        .preview_ace16_insert_rows("Items", &[request], &mut original::budget(), &|| false)
        .err();
    assert_eq!(
        error,
        Some(EditError::Unsupported(WriteBlocker::ConstraintCoverage))
    );
}

#[test]
fn descending_binary_prefix_keys_compare_before_their_row_suffixes() {
    for code in [5, 6] {
        let settings = Settings {
            family: Family::Binary,
            descending: true,
            tuple: false,
            reversed: false,
            unique: false,
            ignored: false,
        };
        let case = model::source(
            code,
            settings,
            vec![
                row(1, None, Some(vec![])),
                row(2, None, Some(vec![255; 8])),
                row(3, None, Some(vec![0])),
            ],
        );
        run(
            &case,
            &[
                row(10, None, Some(vec![255; 255])),
                row(11, None, Some(vec![255; 9])),
                row(12, None, Some(vec![])),
                row(13, None, None),
            ],
        );
    }
}

use super::{checked, corpus, decimal_fixture, numeric_fixture};
use decimal_fixture::Cell;

#[test]
fn only_all_key_components_null_are_omitted_including_reordered_unique_tuples() {
    for code in [1, 2, 5, 6] {
        for descending in [false, true] {
            for unique in [false, true] {
                checked(&corpus::integer_pair(code, descending, unique), 6);
            }
        }
    }
}

#[test]
fn false_boolean_with_another_null_component_is_still_an_eligible_tuple() {
    for code in [1, 2, 5, 6] {
        let rows = [vec![Some(1), Some(0), None], vec![Some(2), Some(-1), None]];
        let mut bytes =
            numeric_fixture::database(code, &[4, 1, 4], &[(1, false), (2, true)], &rows, 0);
        if code == 1 {
            bytes[4..20].copy_from_slice(b"Standard Jet DB\0");
        }
        checked(&corpus::ignoring(bytes, &[], false), rows.len());
    }
}

#[test]
fn single_guid_and_decimal_profiles_distinguish_null_from_zero_payloads() {
    for code in [1, 2, 5, 6] {
        for descending in [false, true] {
            for kind in [15, 16] {
                let zero = if kind == 15 {
                    Cell::Guid([0; 16])
                } else {
                    Cell::Decimal(0)
                };
                let nonzero = if kind == 15 {
                    Cell::Guid([255; 16])
                } else {
                    Cell::Decimal(-9007199254740993)
                };
                let rows = [
                    vec![Some(Cell::Number(1)), None],
                    vec![Some(Cell::Number(2)), Some(zero)],
                    vec![Some(Cell::Number(3)), Some(nonzero)],
                    vec![Some(Cell::Number(4)), None],
                ];
                let bytes =
                    decimal_fixture::database(code, &[4, kind], &[(1, descending)], &rows, 0);
                checked(&corpus::ignoring(bytes, &[0, 3], true), rows.len());
            }
        }
    }
}

#[test]
fn mixed_decimal_guid_and_numeric_tuple_families_keep_partial_null_roots() {
    for code in [1, 2, 5, 6] {
        for decimal in [false, true] {
            let kind = if decimal { 16 } else { 4 };
            let value = if decimal {
                Cell::Decimal(10_001)
            } else {
                Cell::Number(10_001)
            };
            let rows = [
                vec![Some(Cell::Number(1)), None, None],
                vec![Some(Cell::Number(2)), Some(Cell::Guid([0; 16])), None],
                vec![Some(Cell::Number(3)), None, Some(value)],
                vec![
                    Some(Cell::Number(4)),
                    Some(Cell::Guid([255; 16])),
                    Some(value),
                ],
            ];
            let bytes =
                decimal_fixture::database(code, &[4, 15, kind], &[(2, true), (1, false)], &rows, 0);
            checked(&corpus::ignoring(bytes, &[0], true), rows.len());
        }
    }
}

#[test]
fn empty_and_entirely_null_keys_keep_full_table_rows_with_empty_physical_membership() {
    for code in [1, 2, 5, 6] {
        for rows in [
            Vec::new(),
            vec![vec![Some(1), None, None], vec![Some(2), None, None]],
        ] {
            let mut bytes =
                numeric_fixture::database(code, &[4, 4, 4], &[(1, false), (2, true)], &rows, 0);
            if code == 1 {
                bytes[4..20].copy_from_slice(b"Standard Jet DB\0");
            }
            let omitted: Vec<_> = (0..rows.len()).collect();
            checked(&corpus::ignoring(bytes, &omitted, true), rows.len());
        }
    }
}

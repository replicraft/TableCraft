use super::*;
use tablecraft_ace::{
    DecimalIndexSelection, IndexProjectionSelection, IndexReadRequest, TableSnapshot,
    read_table_index_with_request,
};
use tablecraft_core::values::Decimal;

const PAGE_BYTES: usize = 4096;
const ROOT_PAGE: usize = 14;
const LEFT_LEAF: usize = ROOT_PAGE + 1;
const RIGHT_LEAF: usize = ROOT_PAGE + 2;
const USAGE_PAGE: usize = 13;
const TABLE_PAGE: u32 = 10;
const ENTRY_START: usize = 480;
const BITMAP_OFFSET: usize = 27;
const OWNER_OFFSET: usize = 4;
const PREVIOUS_OFFSET: usize = 12;
const NEXT_OFFSET: usize = 16;
const TAIL_OFFSET: usize = 20;
const FREE_OFFSET: usize = 2;
const MAGNITUDE_TAIL: usize = 17;
const MAXIMUM: i128 = 9_999_999_999_999_999_999_999_999_999;

fn selected(
    bytes: &[u8],
    selection: DecimalIndexSelection,
    columns: Option<&[&str]>,
    maximum_rows: u32,
) -> Result<TableSnapshot, InspectError> {
    read_table_index_with_request(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        "Compound",
        IndexReadRequest {
            columns,
            selection: IndexProjectionSelection::Decimal(selection),
            maximum_rows,
        },
        &mut budget(),
        &|| false,
    )
}

#[test]
fn exact_decimal_points_preserve_all_digits_declared_scale_nulls_and_projected_ids() {
    let values = [
        None,
        Some(-MAXIMUM),
        Some(0),
        Some(MAXIMUM),
        Some(1),
        Some(1),
    ];
    for code in [1, 2, 5, 6] {
        for descending in [false, true] {
            for scale in [0, 4, 28] {
                let bytes = fixture::database_for(code, descending, 28, scale, &values, 0);
                let original = bytes.clone();
                for (coefficient, ids) in [
                    (-MAXIMUM, vec![2]),
                    (0, vec![3]),
                    (MAXIMUM, vec![4]),
                    (1, vec![5, 6]),
                    (2, vec![]),
                ] {
                    let selection =
                        DecimalIndexSelection::Equal(Decimal::new(coefficient, scale).unwrap());
                    assert_eq!(
                        selected(&bytes, selection, Some(&["Id"]), 2)
                            .unwrap()
                            .rows(),
                        ids.iter()
                            .map(|id| vec![Value::Int32(*id)])
                            .collect::<Vec<_>>()
                    );
                    assert_eq!(
                        selected(&bytes, selection, Some(&[]), 2).unwrap().rows(),
                        vec![vec![]; ids.len()]
                    );
                    for row in selected(&bytes, selection, None, 2).unwrap().rows() {
                        assert_eq!(
                            row[1],
                            Value::Decimal(Decimal::new(coefficient, scale).unwrap())
                        );
                    }
                }
                assert_eq!(
                    selected(&bytes, DecimalIndexSelection::Null, Some(&["Id"]), 1)
                        .unwrap()
                        .rows(),
                    [vec![Value::Int32(1)]]
                );
                assert!(selected(&bytes, DecimalIndexSelection::All, None, 2).is_err());
                assert_eq!(
                    selected(&bytes, DecimalIndexSelection::All, None, 6)
                        .unwrap()
                        .rows()
                        .len(),
                    6
                );
                assert_eq!(bytes, original);
            }
        }
    }
}

#[test]
fn caller_points_require_the_exact_field_representation_and_single_decimal_profile() {
    let bytes = fixture::database(false, 4, 2, &[], 0);
    for point in [Decimal::new(1, 0).unwrap(), Decimal::new(10000, 2).unwrap()] {
        assert_eq!(
            selected(&bytes, DecimalIndexSelection::Equal(point), Some(&[]), 1).err(),
            Some(InspectError::Unsupported)
        );
    }
    for order in [vec![(0, false)], vec![(0, false), (1, false)]] {
        let bytes = numeric_fixture::database(6, &[4, 4], &order, &[], 0);
        for selection in [
            DecimalIndexSelection::All,
            DecimalIndexSelection::Null,
            DecimalIndexSelection::Equal(Decimal::new(0, 0).unwrap()),
        ] {
            assert_eq!(
                selected(&bytes, selection, Some(&[]), 1).err(),
                Some(InspectError::Unsupported)
            );
        }
    }
}

fn tree(code: u8, descending: bool) -> Vec<u8> {
    let mut bytes = fixture::database_for(
        code,
        descending,
        28,
        4,
        &[
            None,
            Some(-MAXIMUM),
            Some(-1),
            Some(0),
            Some(1),
            Some(MAXIMUM),
        ],
        0,
    );
    bytes.resize((RIGHT_LEAF + 1) * PAGE_BYTES, 0);
    let original = &bytes[ROOT_PAGE * PAGE_BYTES..LEFT_LEAF * PAGE_BYTES];
    let mut entries = Vec::new();
    let mut previous = 0;
    for end in 0..=PAGE_BYTES - ENTRY_START {
        if original[BITMAP_OFFSET + end / 8] & (1 << (end % 8)) != 0 {
            entries.push(original[ENTRY_START + previous..ENTRY_START + end].to_vec());
            previous = end;
        }
    }

    let page = |leaf: bool, entries: &[Vec<u8>], previous: u32, next: u32, tail: u32| {
        let mut raw = vec![0; PAGE_BYTES];
        raw[..2].copy_from_slice(&[if leaf { 4 } else { 3 }, 1]);
        support::put32(&mut raw, OWNER_OFFSET, TABLE_PAGE);
        support::put32(&mut raw, PREVIOUS_OFFSET, previous);
        support::put32(&mut raw, NEXT_OFFSET, next);
        support::put32(&mut raw, TAIL_OFFSET, tail);
        let mut end = 0;
        for entry in entries {
            raw[ENTRY_START + end..ENTRY_START + end + entry.len()].copy_from_slice(entry);
            end += entry.len();
            raw[BITMAP_OFFSET + end / 8] |= 1 << (end % 8);
        }
        support::put16(
            &mut raw,
            FREE_OFFSET,
            (PAGE_BYTES - ENTRY_START - end) as u16,
        );
        raw
    };
    let mut separator = entries[2].clone();
    separator.extend((LEFT_LEAF as u32).to_be_bytes());
    bytes[ROOT_PAGE * PAGE_BYTES..LEFT_LEAF * PAGE_BYTES].copy_from_slice(&page(
        false,
        &[separator],
        0,
        0,
        RIGHT_LEAF as u32,
    ));
    bytes[LEFT_LEAF * PAGE_BYTES..RIGHT_LEAF * PAGE_BYTES].copy_from_slice(&page(
        true,
        &entries[..3],
        0,
        RIGHT_LEAF as u32,
        0,
    ));
    bytes[RIGHT_LEAF * PAGE_BYTES..(RIGHT_LEAF + 1) * PAGE_BYTES].copy_from_slice(&page(
        true,
        &entries[3..],
        LEFT_LEAF as u32,
        0,
        0,
    ));
    bytes[USAGE_PAGE * PAGE_BYTES..ROOT_PAGE * PAGE_BYTES].copy_from_slice(&support::data_page(
        1,
        &[(0, vec![0, 0, 0, 0, 0, 0, 0xc0, 1])],
    ));
    bytes
}

#[test]
fn decimal_equality_and_null_bounds_prune_disjoint_subtrees_in_both_encoding_families() {
    for code in [1, 2, 5, 6] {
        for descending in [false, true] {
            let mut bytes = tree(code, descending);
            assert_eq!(
                selected(&bytes, DecimalIndexSelection::All, None, 6)
                    .unwrap()
                    .rows()
                    .len(),
                6
            );
            let selection = if descending {
                DecimalIndexSelection::Null
            } else {
                DecimalIndexSelection::Equal(Decimal::new(MAXIMUM, 4).unwrap())
            };
            bytes[LEFT_LEAF * PAGE_BYTES + OWNER_OFFSET] = 0;
            assert_eq!(
                selected(&bytes, selection, Some(&[]), 1)
                    .unwrap()
                    .rows()
                    .len(),
                1
            );
            assert!(selected(&bytes, DecimalIndexSelection::All, None, 6).is_err());
        }
    }
}

#[test]
fn compressed_duplicate_points_keep_every_row_and_omitted_keys_still_validate() {
    for code in [1, 2, 5, 6] {
        for descending in [false, true] {
            let point = Decimal::new(-MAXIMUM, 28).unwrap();
            let bytes = fixture::database_for(code, descending, 28, 28, &[Some(-MAXIMUM); 3], 21);
            assert_eq!(
                selected(&bytes, DecimalIndexSelection::Equal(point), Some(&[]), 3)
                    .unwrap()
                    .rows()
                    .len(),
                3
            );
        }
    }
    let mut bytes = fixture::database(false, 28, 4, &[Some(1)], 0);
    // Literal ascending magnitude tail changes the indexed coefficient to 2.
    bytes[ROOT_PAGE * PAGE_BYTES + ENTRY_START + MAGNITUDE_TAIL] = 2;
    assert!(
        selected(
            &bytes,
            DecimalIndexSelection::Equal(Decimal::new(2, 4).unwrap()),
            Some(&[]),
            1
        )
        .is_err()
    );
}

#[test]
fn decimal_point_reads_propagate_cancellation_and_cumulative_resource_errors() {
    let bytes = tree(6, false);
    for cancelled in [false, true] {
        let mut work = if cancelled {
            budget()
        } else {
            ResourceBudget::new(0, 0, 0)
        };
        let error = read_table_index_with_request(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            "Compound",
            IndexReadRequest {
                columns: Some(&[]),
                selection: IndexProjectionSelection::Decimal(DecimalIndexSelection::Null),
                maximum_rows: 1,
            },
            &mut work,
            &|| cancelled,
        )
        .err()
        .unwrap();
        if cancelled {
            assert_eq!(error, InspectError::Cancelled);
        } else {
            assert!(matches!(error, InspectError::Limit(_)));
        }
    }
}

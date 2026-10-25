use super::*;
use tablecraft_ace::{
    BooleanIndexSelection, IndexProjectionSelection, IndexReadRequest, TableSnapshot,
    read_table_index_with_request,
};

const PAGE_BYTES: usize = 4096;
const ROOT_PAGE: usize = 14;
const LEFT_LEAF_PAGE: usize = ROOT_PAGE + 1;
const RIGHT_LEAF_PAGE: usize = ROOT_PAGE + 2;
const INDEX_USAGE_PAGE: usize = 13;
const TABLE_PAGE: u32 = 10;
const ENTRY_START: usize = 480;
const BITMAP_OFFSET: usize = 27;
const OWNER_OFFSET: usize = 4;
const PREVIOUS_OFFSET: usize = 12;
const NEXT_OFFSET: usize = 16;
const TAIL_OFFSET: usize = 20;
const FREE_OFFSET: usize = 2;

fn selected(
    bytes: &[u8],
    value: BooleanIndexSelection,
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
            selection: IndexProjectionSelection::Boolean(value),
            maximum_rows,
        },
        &mut budget(),
        &|| false,
    )
}

#[test]
fn exact_boolean_seeks_project_validate_and_cap_only_selected_rows() {
    for code in [1, 2, 5, 6] {
        for descending in [false, true] {
            let mut bytes = database(code, descending, &[false, true, false, true], 0);
            if code == 1 {
                bytes[4..20].copy_from_slice(b"Standard Jet DB\0");
            }
            let original = bytes.clone();
            for value in [false, true] {
                let selection = BooleanIndexSelection::Equal(value);
                let expected = if value { [2, 4] } else { [1, 3] };
                assert_eq!(
                    selected(&bytes, selection, Some(&["Id"]), 2)
                        .unwrap()
                        .rows(),
                    expected.map(|id| vec![Value::Int32(id)])
                );
                assert_eq!(
                    selected(&bytes, selection, Some(&[]), 2).unwrap().rows(),
                    [vec![], vec![]]
                );
                assert!(selected(&bytes, selection, None, 1).is_err());
            }
            assert!(selected(&bytes, BooleanIndexSelection::All, None, 2).is_err());
            assert_eq!(
                selected(&bytes, BooleanIndexSelection::All, None, 4)
                    .unwrap()
                    .rows()
                    .len(),
                4
            );
            assert_eq!(bytes, original);
        }
    }
}

#[test]
fn boolean_selections_reject_other_carriers_and_composites_even_when_empty() {
    for kinds in [&[4, 2][..], &[4, 1][..]] {
        let order = if kinds[1] == 1 {
            vec![(0, false), (1, false)]
        } else {
            vec![(1, false)]
        };
        let bytes = numeric_fixture::database(6, kinds, &order, &[], 0);
        for selection in [
            BooleanIndexSelection::All,
            BooleanIndexSelection::Equal(false),
        ] {
            assert_eq!(
                selected(&bytes, selection, Some(&[]), 1).err(),
                Some(InspectError::Unsupported)
            );
        }
    }
    for value in [false, true] {
        let bytes = database(6, false, &[!value], 0);
        assert!(
            selected(&bytes, BooleanIndexSelection::Equal(value), None, 1)
                .unwrap()
                .rows()
                .is_empty()
        );
    }
}

fn tree(descending: bool) -> Vec<u8> {
    let mut bytes = database(6, descending, &[false, true, false, true], 0);
    bytes.resize((RIGHT_LEAF_PAGE + 1) * PAGE_BYTES, 0);
    let original = &bytes[ROOT_PAGE * PAGE_BYTES..(ROOT_PAGE + 1) * PAGE_BYTES];
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
    let mut separator = entries[1].clone();
    separator.extend((LEFT_LEAF_PAGE as u32).to_be_bytes());
    bytes[ROOT_PAGE * PAGE_BYTES..LEFT_LEAF_PAGE * PAGE_BYTES].copy_from_slice(&page(
        false,
        &[separator],
        0,
        0,
        RIGHT_LEAF_PAGE as u32,
    ));
    bytes[LEFT_LEAF_PAGE * PAGE_BYTES..RIGHT_LEAF_PAGE * PAGE_BYTES].copy_from_slice(&page(
        true,
        &entries[..2],
        0,
        RIGHT_LEAF_PAGE as u32,
        0,
    ));
    bytes[RIGHT_LEAF_PAGE * PAGE_BYTES..(RIGHT_LEAF_PAGE + 1) * PAGE_BYTES].copy_from_slice(&page(
        true,
        &entries[2..],
        LEFT_LEAF_PAGE as u32,
        0,
        0,
    ));
    bytes[INDEX_USAGE_PAGE * PAGE_BYTES..ROOT_PAGE * PAGE_BYTES].copy_from_slice(
        &support::data_page(1, &[(0, vec![0, 0, 0, 0, 0, 0, 0xc0, 1])]),
    );
    bytes
}

#[test]
fn boolean_bounds_prune_disjoint_branches_without_qualifying_unvisited_corruption() {
    for descending in [false, true] {
        let mut bytes = tree(descending);
        assert_eq!(
            selected(&bytes, BooleanIndexSelection::All, None, 4)
                .unwrap()
                .rows()
                .len(),
            4
        );
        bytes[LEFT_LEAF_PAGE * PAGE_BYTES + OWNER_OFFSET] = 0;
        assert_eq!(
            selected(&bytes, BooleanIndexSelection::Equal(descending), None, 2)
                .unwrap()
                .rows()
                .len(),
            2
        );
        assert!(selected(&bytes, BooleanIndexSelection::All, None, 4).is_err());
        assert!(selected(&bytes, BooleanIndexSelection::Equal(!descending), None, 2).is_err());
    }
}

#[test]
fn omitted_boolean_keys_still_validate_and_shared_limits_cancel_before_publication() {
    let mut bytes = database(6, false, &[true], 0);
    bytes[ROOT_PAGE * PAGE_BYTES + ENTRY_START + 1] = 255;
    assert!(selected(&bytes, BooleanIndexSelection::Equal(false), Some(&[]), 1).is_err());

    let bytes = tree(false);
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
                selection: IndexProjectionSelection::Boolean(BooleanIndexSelection::Equal(false)),
                maximum_rows: 2,
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

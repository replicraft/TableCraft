use super::*;
use crate::{Cell, CellKind, State};

fn page() -> Data {
    Data {
        source_revision: Some(7),
        row_ordinals: Some(RowOrdinals::new(vec![7, 100]).unwrap()),
        table: "Items".into(),
        offset: 50,
        columns: vec!["Value".into()],
        rows: vec![vec![Cell::scalar("first")], vec![Cell::scalar("second")]],
        has_more: true,
    }
}

#[test]
fn sparse_subset_positions_do_not_become_native_identities() {
    let data = page();
    assert_eq!(data.validate_row_ordinals(), Ok(()));
    assert_eq!(data.parent_ordinal(0), Some(7));
    assert_eq!(data.parent_ordinal(1), Some(100));
    assert_eq!(data.parent_ordinal(2), None);
    assert_eq!(data.display_ordinal(0), Some(50));
    assert_eq!(data.display_ordinal(1), Some(51));
    assert_eq!(data.row_index(100), Some(1));
    assert_eq!(data.row_index(50), None);
    assert_eq!(data.row_index(8), None);
    assert_eq!(data.edit_target(1, 0).unwrap().parent_ordinal, 100);
}

#[test]
fn malformed_membership_refuses_without_contiguous_fallback() {
    assert_eq!(RowOrdinals::new(vec![4, 4]), Err(RowOrdinalError::Order));
    assert_eq!(RowOrdinals::new(vec![9, 4]), Err(RowOrdinalError::Order));

    let mut data = page();
    data.row_ordinals = Some(RowOrdinals::new(vec![50]).unwrap());
    assert_eq!(data.validate_row_ordinals(), Err(RowOrdinalError::Count));
    assert_eq!(data.parent_ordinal(0), None);
    assert_eq!(data.row_index(50), None);
    assert_eq!(data.edit_target(0, 0), None);

    data.row_ordinals = Some(RowOrdinals::new(vec![7, 100]).unwrap());
    data.source_revision = None;
    assert_eq!(data.validate_row_ordinals(), Err(RowOrdinalError::Source));
    assert_eq!(data.parent_ordinal(0), None);

    data.row_ordinals = None;
    assert_eq!(data.validate_row_ordinals(), Ok(()));
    assert_eq!(data.parent_ordinal(0), None);
    data.source_revision = Some(7);
    data.offset = u32::MAX;
    assert_eq!(data.validate_row_ordinals(), Err(RowOrdinalError::Overflow));
    assert_eq!(data.parent_ordinal(0), None);
}

#[test]
fn rejected_publication_keeps_previous_page_and_query_has_no_native_identity() {
    let mut state = State::default();
    state.set_table_data(page());
    let mut invalid = page();
    invalid.rows.pop();
    state.set_table_data(invalid);
    assert_eq!(state.data.as_ref().unwrap().rows.len(), 2);
    assert!(state.error.is_some());

    state.set_query_data(None, page());
    assert!(state.query_data.is_none());
    assert!(state.error.is_some());

    let mut query = page();
    query.row_ordinals = None;
    query.source_revision = None;
    state.set_query_data(None, query);
    assert!(state.query_data.is_some());
    assert_eq!(state.query_data.as_ref().unwrap().parent_ordinal(0), None);
}

#[test]
fn collection_targets_use_membership_not_display_offset() {
    let mut data = page();
    data.rows[1][0].kind = CellKind::Multivalue {
        assigned: true,
        members: 1,
    };
    assert_eq!(data.contents_target(1, 0).unwrap().parent_ordinal, 100);
    data.row_ordinals = Some(RowOrdinals::new(vec![7]).unwrap());
    assert!(data.contents_target(1, 0).is_none());
}

#[test]
fn display_order_keeps_native_targets_and_indexed_reverse_lookup_separate() {
    let mut data = page();
    data.row_ordinals = Some(RowOrdinals::from_display_order(vec![7, 1, 4]).unwrap());
    data.rows.push(vec![Cell::scalar("third")]);
    assert_eq!(data.validate_row_ordinals(), Ok(()));
    assert_eq!(data.row_ordinals.as_ref().unwrap().as_slice(), [7, 1, 4]);
    assert!(!data.row_ordinals.as_ref().unwrap().is_source_ordered());
    for (row, ordinal) in [7, 1, 4].into_iter().enumerate() {
        assert_eq!(data.parent_ordinal(row), Some(ordinal));
        assert_eq!(data.row_index(ordinal), Some(row));
        assert_eq!(data.display_ordinal(row), Some(50 + row as u32));
        assert_eq!(data.edit_target(row, 0).unwrap().parent_ordinal, ordinal);
    }
    assert_eq!(data.row_index(2), None);
    data.rows[2][0].kind = CellKind::Multivalue {
        assigned: true,
        members: 1,
    };
    assert_eq!(data.contents_target(2, 0).unwrap().parent_ordinal, 4);

    data.rows.pop();
    assert_eq!(data.validate_row_ordinals(), Err(RowOrdinalError::Count));
    assert_eq!(data.row_index(7), None);
    assert_eq!(data.parent_ordinal(0), None);

    let mut state = State::default();
    state.set_query_data(None, data);
    assert!(state.query_data.is_none());
}

#[test]
fn arbitrary_order_rejects_duplicates_and_caps_lookup_storage() {
    assert_eq!(
        RowOrdinals::from_display_order(vec![7, 1, 7]),
        Err(RowOrdinalError::Duplicate),
    );
    assert_eq!(RowOrdinals::new(vec![7, 1, 4]), Err(RowOrdinalError::Order));
    assert_eq!(
        RowOrdinals::from_display_order(vec![1, 4, 7]).unwrap(),
        RowOrdinals::new(vec![1, 4, 7]).unwrap(),
    );

    // The caller-owned values alone exceed the independent eight MiB UI limit.
    let over_limit = vec![0; 2_097_153];
    assert_eq!(RowOrdinals::new(over_limit), Err(RowOrdinalError::Limit));
    // A permutation also accounts for its inverse before allocating that lookup.
    let permutation: Vec<_> = (0..1_000_000).rev().collect();
    assert_eq!(
        RowOrdinals::from_display_order(permutation),
        Err(RowOrdinalError::Limit),
    );
}

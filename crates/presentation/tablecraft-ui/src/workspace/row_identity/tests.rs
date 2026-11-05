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

    state.set_query_data(page());
    assert!(state.query_data.is_none());
    assert!(state.error.is_some());

    let mut query = page();
    query.row_ordinals = None;
    query.source_revision = None;
    state.set_query_data(query);
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

use super::*;
use crate::{Action, Cell, Database, RowOrdinals, Table};
use egui::accesskit::Role;
use egui_kittest::{Harness, kittest::Queryable};

fn state() -> State {
    let mut state = State::default();
    state.dismiss_backstage();
    state.database = Some(Database {
        document_revision: 7,
        title_path: None,
        title: "Selection filters".into(),
        format: "ACE 16".into(),
        tables: vec![Table {
            name: "People".into(),
            system: false,
            storage: "Local".into(),
        }],
        queries: Vec::new(),
    });
    state.set_table_data(Data {
        source_revision: Some(7),
        row_ordinals: None,
        table: "People".into(),
        offset: 0,
        columns: vec!["ID".into(), "Name".into()],
        rows: vec![vec!["0".into(), "Ada".into()]],
        has_more: true,
    });
    state
}

fn publication(id: u64, enabled: bool, offset: u32, ordinals: Vec<u32>) -> FilterPublication {
    let count = if enabled { 3 } else { 10 };
    let end = offset + ordinals.len() as u32;
    let rows = ordinals
        .iter()
        .map(|ordinal| vec![Cell::scalar(ordinal.to_string()), Cell::scalar("Ada")])
        .collect();
    FilterPublication {
        sort: None,
        filter_count: 1,
        target: FilterTarget {
            document_revision: 7,
            table: "People".into(),
            filter_id: id,
        },
        enabled,
        matched_rows: 3,
        source_rows: 10,
        data: Data {
            source_revision: Some(7),
            row_ordinals: Some(RowOrdinals::new(ordinals).unwrap()),
            table: "People".into(),
            offset,
            columns: vec!["ID".into(), "Name".into()],
            rows,
            has_more: end < count,
        },
    }
}

fn apply(state: &mut State) -> FilterTarget {
    let selection = state.current_filter_selection().unwrap();
    assert!(state.begin_filter_selection(1, selection, FilterComparison::Equal));
    assert!(state.set_filtered_data(1, publication(20, true, 0, vec![2, 5])));
    state.active_filter_target().unwrap()
}

#[test]
fn filter_publication_checks_sparse_membership_counts_request_and_retained_toggle() {
    let mut state = state();
    let selection = state.current_filter_selection().unwrap();
    assert!(state.begin_filter_selection(1, selection, FilterComparison::Equal));
    assert!(!state.close_table(7, "People"));
    assert!(!state.set_filtered_data(2, publication(20, true, 0, vec![2, 5])));
    let mut malformed = publication(20, true, 0, vec![2, 5]);
    malformed.data.has_more = false;
    assert!(!state.set_filtered_data(1, malformed));
    assert!(state.active_filter_target().is_none());
    assert_eq!(state.data.as_ref().unwrap().parent_ordinal(0), Some(0));

    let target = apply(&mut state);
    assert_eq!(state.data.as_ref().unwrap().parent_ordinal(0), Some(2));
    let selected = state.current_filter_selection().unwrap();
    assert_eq!(selected.parent_ordinal, 2);
    assert_eq!(selected.filter_id, Some(20));
    assert!(state.can_filter_page(&target, 2, true));
    assert!(!state.can_filter_page(&target, 3, true));
    assert!(state.begin_filter_page(3, target.clone(), 2, true));
    assert!(state.set_filtered_data(3, publication(20, true, 2, vec![9])));
    assert_eq!(state.data.as_ref().unwrap().display_ordinal(0), Some(2));
    assert_eq!(state.data.as_ref().unwrap().parent_ordinal(0), Some(9));

    assert!(state.begin_filter_page(4, target.clone(), 0, false));
    assert!(state.set_filtered_data(4, publication(20, false, 0, vec![0, 1])));
    assert!(!state.table_filter_enabled());
    assert_eq!(state.active_filter_target(), Some(target.clone()));
    assert_eq!(
        state.current_filter_selection().unwrap().filter_id,
        Some(20)
    );
    assert!(!state.current_filter_selection().unwrap().enabled);
    state.select_view(DocumentView::Query);
    assert_eq!(state.active_filter_target(), Some(target));
    assert!(!state.can_clear_filter(&state.active_filter_target().unwrap()));
}

#[test]
fn zero_matches_keep_columns_toggle_clear_and_failures_retain_previous_filter() {
    let mut state = state();
    let selection = state.current_filter_selection().unwrap();
    assert!(state.begin_filter_selection(1, selection, FilterComparison::NotEqual));
    let mut empty = publication(20, true, 0, Vec::new());
    empty.matched_rows = 0;
    empty.data.has_more = false;
    assert!(state.set_filtered_data(1, empty));
    let target = state.active_filter_target().unwrap();
    assert_eq!(state.data.as_ref().unwrap().columns.len(), 2);
    assert!(state.data.as_ref().unwrap().rows.is_empty());
    assert!(state.current_filter_selection().is_none());
    assert!(state.can_filter_page(&target, 0, false));
    assert!(state.can_clear_filter(&target));
    assert!(state.begin_clear_filter(2, target.clone()));
    state.fail_filter(2, "Read failure: existing criteria retained.".into());
    assert_eq!(
        state.filter_error_message(),
        Some("Read failure: existing criteria retained.")
    );
    let database = state.database.take();
    assert!(state.filter_error_message().is_none());
    state.database = database;
    let data = state.data.take();
    assert!(state.filter_error_message().is_none());
    state.data = data;
    assert_eq!(
        state.filter_error_message(),
        Some("Read failure: existing criteria retained.")
    );
    state.select_view(DocumentView::Query);
    assert_eq!(
        state.filter_error_message(),
        Some("Read failure: existing criteria retained.")
    );
    state.select_view(DocumentView::Table);
    assert_eq!(state.active_filter_target(), Some(target.clone()));
    assert!(state.data.as_ref().unwrap().rows.is_empty());

    assert!(state.begin_clear_filter(3, target.clone()));
    let mut wrong = publication(30, false, 0, vec![0, 1]);
    wrong.filter_count = 0;
    wrong.matched_rows = wrong.source_rows;
    wrong.data.row_ordinals = None;
    assert!(!state.set_filtered_data(3, wrong));
    assert_eq!(state.active_filter_target(), Some(target.clone()));
    assert!(state.begin_clear_filter(4, target));
    let mut cleared = publication(30, false, 0, vec![0, 1]);
    cleared.filter_count = 0;
    cleared.matched_rows = cleared.source_rows;
    assert!(state.set_filtered_data(4, cleared));
    let target = state.active_filter_target().unwrap();
    assert_eq!(target.filter_id, 30);
    assert!(!state.can_clear_filter(&target));
    assert!(!state.can_filter_page(&target, 0, true));
    assert!(state.data.as_ref().unwrap().row_ordinals.is_some());
}

#[test]
fn restore_uses_fresh_catalog_revision_and_accepts_clamped_new_membership_only() {
    let mut state = state();
    state.data = None;
    let restore = FilterRestore {
        document_revision: 7,
        table: "People".into(),
        enabled: true,
        offset: 100,
        sort: Some(SortSpec {
            column: "Name".into(),
            descending: true,
        }),
        filter_count: 1,
    };
    assert!(state.begin_filter_restore(9, restore.clone()));
    state.fail_filter(9, "Restore failed for the freshly opened table.".into());
    assert_eq!(
        state.filter_error_message(),
        Some("Restore failed for the freshly opened table.")
    );
    let database = state.database.take();
    assert!(state.filter_error_message().is_none());
    state.database = database;
    state.database.as_mut().unwrap().document_revision = 8;
    assert!(state.filter_error_message().is_none());
    state.database.as_mut().unwrap().document_revision = 7;
    state.select_view(DocumentView::Query);
    assert_eq!(
        state.filter_error_message(),
        Some("Restore failed for the freshly opened table.")
    );
    assert!(state.begin_filter_restore(10, restore.clone()));
    assert!(!state.set_filtered_data(9, publication(20, true, 0, vec![2, 5])));
    assert!(!state.set_filtered_data(10, publication(20, true, 0, vec![2, 5])));
    assert!(
        state.data.is_none(),
        "a reply must not silently drop the captured sort"
    );
    assert!(state.begin_filter_restore(11, restore.clone()));
    let mut wrong_count = publication(20, true, 0, vec![2, 5]);
    wrong_count.sort = restore.sort.clone();
    wrong_count.filter_count = 2;
    assert!(!state.set_filtered_data(11, wrong_count));
    assert!(
        state.data.is_none(),
        "a reply must retain the captured predicate count"
    );
    assert!(state.begin_filter_restore(12, restore.clone()));
    let mut restored = publication(20, true, 0, vec![2, 5]);
    restored.sort = restore.sort;
    assert!(state.set_filtered_data(12, restored));
    assert_eq!(state.active_filter_target().unwrap().filter_id, 20);
    assert_eq!(
        state
            .data
            .as_ref()
            .unwrap()
            .row_ordinals
            .as_ref()
            .unwrap()
            .as_slice(),
        &[2, 5]
    );

    let target = state.active_filter_target().unwrap();
    assert!(state.begin_filter_page(13, target, 2, true));
    state.formatting.date = tablecraft_display::DateFormat::Iso;
    assert!(!state.accepts_filter_reply(13));
    state.validate_table_filter();
    assert!(!state.table_filter_pending());
    assert_eq!(state.data.as_ref().unwrap().offset, 0);
}

#[test]
fn actual_null_menu_and_filtered_navigation_dispatch_runtime_targets_not_page_filters() {
    let mut state = state();
    state.data.as_mut().unwrap().rows[0][0].kind = CellKind::Scalar {
        is_null: true,
        stored_calculation: false,
        boolean: None,
        display_control: crate::DisplayControl::Absent,
    };
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1280.0, 820.0))
        .build_ui_state(
            |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
            (state, Vec::new()),
        );
    crate::configure(&harness.ctx);
    harness.run();
    harness
        .get_by_role_and_label(Role::Button, "Selection")
        .click_accesskit();
    harness.run();
    assert!(harness.query_by_label("Equals selected value").is_none());
    harness
        .get_by_role_and_label(Role::Button, "Is Null")
        .click_accesskit();
    harness.run();
    assert!(
        matches!(harness.state().1.as_slice(), [Action::FilterSelection { target, comparison: FilterComparison::Equal }]
            if target.document_revision == 7 && target.parent_ordinal == 0 && target.column == "ID"
        )
    );
    harness.state_mut().1.clear();
    let state = &mut harness.state_mut().0;
    let selection = state.current_filter_selection().unwrap();
    assert!(state.begin_filter_selection(1, selection, FilterComparison::Equal));
    assert!(state.set_filtered_data(1, publication(20, true, 0, vec![2, 5])));
    harness.run();
    harness
        .get_by_role_and_label(Role::Button, "Next page ›")
        .click_accesskit();
    harness.run();
    assert!(
        matches!(harness.state().1.as_slice(), [Action::FilterPage { target, offset: 2, enabled: true }]
            if target.filter_id == 20
        )
    );
    harness.state_mut().1.clear();
    harness
        .get_by_role_and_label(Role::Button, "Toggle Filter")
        .click_accesskit();
    harness.run();
    assert!(
        matches!(harness.state().1.as_slice(), [Action::FilterPage { target, offset: 0, enabled: false }]
            if target.filter_id == 20
        )
    );
}

fn sorted_publication(
    id: u64,
    sort: Option<SortSpec>,
    enabled: bool,
    filtered: bool,
) -> FilterPublication {
    let mut result = publication(id, enabled, 0, Vec::new());
    result.filter_count = u32::from(filtered);
    result.matched_rows = if filtered { 3 } else { result.source_rows };
    result.sort = sort;
    let ordinals = if result.sort.is_some() {
        vec![9, 5]
    } else {
        vec![0, 1]
    };
    result.data.rows = ordinals
        .iter()
        .map(|ordinal| {
            vec![
                Cell::scalar(ordinal.to_string()),
                Cell::scalar(ordinal.to_string()),
            ]
        })
        .collect();
    result.data.row_ordinals = Some(RowOrdinals::from_display_order(ordinals).unwrap());
    result.data.has_more = true;
    result
}

#[test]
fn native_sort_requires_exact_witness_and_preserves_edit_identity_without_query_substitute() {
    let mut state = state();
    state.sql = "SELECT 42;".into();
    let target = state.current_sort_target().unwrap();
    let sort = SortSpec {
        column: "Name".into(),
        descending: true,
    };
    assert!(state.begin_sort_view(10, target.clone(), Some(sort.clone()), false));
    let wrong = sorted_publication(20, None, false, false);
    assert!(!state.set_filtered_data(10, wrong));
    assert_eq!(state.data.as_ref().unwrap().parent_ordinal(0), Some(0));
    assert!(state.begin_sort_view(11, target, Some(sort.clone()), false));
    assert!(state.set_filtered_data(11, sorted_publication(20, Some(sort), false, false)));
    assert!(state.view == DocumentView::Table);
    assert!(state.query_data.is_none());
    assert_eq!(state.sql, "SELECT 42;");
    assert_eq!(state.current_delete_target().unwrap().parent_ordinal, 9);
    assert_eq!(state.current_filter_selection().unwrap().parent_ordinal, 9);
    assert_eq!(state.table_filter_count(), 0);
    assert!(!state.table_filter_enabled());
    assert!(!state.can_clear_filter(&state.active_filter_target().unwrap()));
    let target = state.current_sort_target().unwrap();
    assert!(state.begin_sort_view(12, target, None, false));
    assert!(state.set_filtered_data(12, sorted_publication(21, None, false, false)));
    assert!(state.table_sort().is_none());
    assert!(
        state
            .data
            .as_ref()
            .unwrap()
            .row_ordinals
            .as_ref()
            .unwrap()
            .is_source_ordered()
    );
}

#[test]
fn clearing_filters_and_off_selection_retain_sort_but_do_not_reapply_inactive_criteria() {
    let mut state = state();
    apply(&mut state);
    let sort = SortSpec {
        column: "Name".into(),
        descending: true,
    };
    let target = state.current_sort_target().unwrap();
    assert!(state.begin_sort_view(10, target, Some(sort.clone()), true));
    assert!(state.set_filtered_data(10, sorted_publication(21, Some(sort.clone()), true, true)));
    let target = state.active_filter_target().unwrap();
    assert!(state.begin_filter_page(11, target.clone(), 0, false));
    assert!(state.set_filtered_data(11, sorted_publication(21, Some(sort.clone()), false, true)));
    let selection = state.current_filter_selection().unwrap();
    assert_eq!(selection.filter_id, Some(21));
    assert!(!selection.enabled);
    assert!(state.begin_clear_filter(12, target));
    assert!(state.set_filtered_data(12, sorted_publication(22, Some(sort.clone()), false, false)));
    assert_eq!(state.table_sort(), Some(sort));
    assert_eq!(state.table_filter_count(), 0);
    assert!(!state.can_filter_page(&state.active_filter_target().unwrap(), 0, true));
}

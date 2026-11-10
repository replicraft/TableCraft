use super::*;
use crate::{Action, Cell, CellKind, Database, Table};
use egui::accesskit::Role;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};

pub(super) fn state() -> State {
    let mut state = State::default();
    state.dismiss_backstage();
    state.find_available = true;
    state.database = Some(Database {
        document_revision: 7,
        title_path: None,
        title: "Find".into(),
        format: "ACE 16".into(),
        tables: vec![Table {
            local: true,
            name: "People".into(),
            system: false,
            storage: "Local".into(),
        }],
        queries: Vec::new(),
    });
    state.set_table_data(page(100));
    state
        .table_selection
        .select(state.data.as_ref().unwrap(), 0, 1);
    state.open_find();
    state.find.as_mut().unwrap().pattern = "Ada".into();
    state
}

pub(super) fn page(offset: u32) -> Data {
    Data {
        row_ordinals: None,
        source_revision: Some(7),
        table: "People".into(),
        offset,
        columns: vec!["ID".into(), "Name".into()],
        rows: vec![vec![
            Cell::scalar(offset.to_string()),
            Cell::scalar("Ada Ada"),
        ]],
        has_more: true,
    }
}

#[test]
fn find_replies_bind_exact_options_publication_and_native_occurrence() {
    let mut oversized = state();
    oversized.find.as_mut().unwrap().pattern = "🦀".repeat(65_536);
    let request = oversized.current_find_request().unwrap();
    assert!(
        !oversized.can_find_next(&request),
        "textbox character policy rejects the complete oversized pattern"
    );

    let mut state = state();
    let request = state.current_find_request().unwrap();
    assert_eq!(request.scope, FindScope::Field("Name".into()));
    assert_eq!(request.matching, FindMatch::Whole);
    assert_eq!(request.direction, FindDirection::All);
    assert!(!request.match_case);
    assert!(!request.formatted);
    state.begin_find(1, request.clone());
    assert!(!state.can_find_next(&request));
    assert!(!state.accepts_find_reply(2, &request));
    let mut altered = request.clone();
    altered.matching = FindMatch::Any;
    assert!(!state.accepts_find_reply(1, &altered));
    assert!(!state.set_find_result(
        1,
        request.clone(),
        Some(FindPosition {
            parent_ordinal: 200,
            column: "Name".into(),
            span: Some(FindSpan {
                start_utf16: 0,
                end_utf16: 3
            }),
        }),
        Some(page(199))
    ));
    assert_eq!(state.data.as_ref().unwrap().offset, 100);

    state.begin_find(3, request.clone());
    assert!(state.set_find_result(
        3,
        request,
        Some(FindPosition {
            parent_ordinal: 200,
            column: "Name".into(),
            span: Some(FindSpan {
                start_utf16: 0,
                end_utf16: 3
            }),
        }),
        Some(page(200))
    ));
    assert_eq!(state.table_selection.position(), Some((0, 1)));
    let next = state.current_find_request().unwrap();
    assert_eq!(
        next.start.as_ref().unwrap().span,
        Some(FindSpan {
            start_utf16: 0,
            end_utf16: 3,
        })
    );
    state.begin_find(4, next.clone());
    assert!(state.set_find_result(4, next, None, None));
    assert_eq!(state.data.as_ref().unwrap().offset, 200);
    assert_eq!(state.table_selection.position(), Some((0, 1)));
}

#[test]
fn new_user_selection_source_or_cancel_prevents_stale_find_publication() {
    let mut state = state();
    let request = state.current_find_request().unwrap();
    state.begin_find(1, request.clone());
    state
        .table_selection
        .select(state.data.as_ref().unwrap(), 0, 0);
    assert!(!state.accepts_find_reply(1, &request));
    state.fail_find(1, "Stale failure".into());
    assert!(state.find.as_ref().unwrap().error.is_none());
    assert_eq!(state.close_find(), Some(1));
    assert!(!state.set_find_result(1, request, None, None));

    state.open_find();
    state.find.as_mut().unwrap().pattern = "Ada".into();
    let request = state.current_find_request().unwrap();
    state.begin_find(2, request.clone());
    state.set_table_data(page(100));
    assert!(!state.accepts_find_reply(2, &request));
    assert_eq!(state.data.as_ref().unwrap().offset, 100);

    state.data.as_mut().unwrap().rows[0][0].kind = CellKind::Multivalue {
        assigned: true,
        members: 1,
    };
    assert!(!state.can_open_find());
    assert!(state.current_find_request().is_none());
}

#[test]
fn actual_find_controls_are_modeless_and_only_submit_explicit_checked_requests() {
    let mut harness = Harness::builder()
        .with_size(egui::vec2(900.0, 580.0))
        .build_ui_state(
            |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
            (state(), Vec::new()),
        );
    crate::configure(&harness.ctx);
    harness.run();
    assert!(
        harness
            .get_by_role_and_label(Role::TextInput, "Find What:")
            .is_focused()
    );
    assert!(
        harness
            .get_by_role_and_label(Role::CheckBox, "Search Fields As Formatted")
            .accesskit_node()
            .is_disabled()
    );
    assert!(harness.state().1.is_empty());
    let state = &mut harness.state_mut().0;
    state
        .table_selection
        .select(state.data.as_ref().unwrap(), 0, 0);
    harness.run();
    assert_eq!(
        harness.state().0.current_find_request().unwrap().scope,
        FindScope::Field("ID".into())
    );
    harness
        .get_by_role_and_label(Role::Button, "Find Next")
        .click_accesskit();
    harness.run();
    assert!(
        matches!(harness.state().1.as_slice(), [Action::FindNext { request }]
            if request.target.document_revision == 7 && request.target.table == "People"
                && request.scope == FindScope::Field("ID".into()) && request.pattern == "Ada"
        )
    );
    harness
        .get_by_role_and_label(Role::Button, "Cancel")
        .click_accesskit();
    harness.run();
    assert!(!harness.state().0.find_window_open());
    assert_eq!(harness.state().0.data.as_ref().unwrap().offset, 100);
}

#[test]
fn command_find_opens_existing_workflow_and_zoom_cancels_pending_find_without_publication() {
    let mut state = state();
    state.close_find();
    let mut harness = Harness::builder()
        .with_size(egui::vec2(900.0, 580.0))
        .build_ui_state(
            |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
            (state, Vec::new()),
        );
    crate::configure(&harness.ctx);
    harness.run();
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::F);
    harness.run();
    assert!(harness.state().0.find_window_open());
    harness
        .get_by_role_and_label(Role::TextInput, "Find What:")
        .type_text("Ada");
    harness.run();
    let request = harness.state().0.current_find_request().unwrap();
    harness.state_mut().0.begin_find(40, request.clone());
    assert!(harness.state_mut().0.open_zoom_cell());
    harness.run();
    assert!(harness.state().0.zoom_dialog_open());
    assert!(!harness.state().0.find_window_open());
    assert!(
        harness
            .state()
            .1
            .iter()
            .any(|action| { matches!(action, Action::CancelFind { request_id: 40 }) })
    );
    assert!(
        !harness
            .state_mut()
            .0
            .set_find_result(40, request, None, None)
    );
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::F);
    harness.run();
    assert!(!harness.state().0.find_window_open());
    assert_eq!(harness.state().0.data.as_ref().unwrap().offset, 100);
}

#[test]
fn escape_dismisses_the_find_popup_before_closing_the_modeless_window() {
    let mut harness = Harness::builder()
        .with_size(egui::vec2(900.0, 580.0))
        .with_max_steps(32)
        .build_ui_state(
            |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
            (state(), Vec::new()),
        );
    crate::configure(&harness.ctx);
    harness.run();
    // Target Find's named control, independently of other visible Ribbon combos.
    // A plain button would miss Escape ownership while inspecting its options.
    harness
        .get_by_role_and_label(Role::ComboBox, "Match:")
        .click_accesskit();
    harness.run();
    assert!(egui::Popup::is_any_open(&harness.ctx));
    harness.key_press(egui::Key::Escape);
    harness.run();
    assert!(!egui::Popup::is_any_open(&harness.ctx));
    assert!(harness.state().0.find_window_open());
    assert!(harness.state().1.is_empty());
    assert_eq!(harness.state().0.find.as_ref().unwrap().pattern, "Ada");
    harness.key_press(egui::Key::Escape);
    harness.run();
    assert!(!harness.state().0.find_window_open());
    assert!(harness.state().1.is_empty());
    assert_eq!(harness.state().0.data.as_ref().unwrap().offset, 100);
}

#[test]
fn filtered_find_republishes_exact_view_and_selects_physical_member_without_losing_filter() {
    let mut state = state();
    state.close_find();
    let selection = state.current_filter_selection().unwrap();
    assert!(state.begin_filter_selection(30, selection, super::super::FilterComparison::Equal));
    let target = FilterTarget {
        document_revision: 7,
        table: "People".into(),
        filter_id: 9,
    };
    let publication = |offset, ordinals: Vec<u32>, has_more| {
        let mut data = page(offset);
        data.rows = ordinals
            .iter()
            .map(|ordinal| vec![Cell::scalar(ordinal.to_string()), Cell::scalar("Ada Ada")])
            .collect();
        data.row_ordinals = Some(crate::RowOrdinals::new(ordinals).unwrap());
        data.has_more = has_more;
        FilterPublication {
            target: target.clone(),
            enabled: true,
            sort: None,
            filter_count: 1,
            matched_rows: 3,
            source_rows: 300,
            data,
        }
    };
    assert!(state.set_filtered_data(30, publication(0, vec![100], true)));
    state.open_find();
    state.find.as_mut().unwrap().pattern = "Ada".into();
    let request = state.current_find_request().unwrap();
    state.begin_find(31, request.clone());
    let found = FindPosition {
        parent_ordinal: 250,
        column: "Name".into(),
        span: Some(FindSpan {
            start_utf16: 0,
            end_utf16: 3,
        }),
    };
    assert!(!state.set_find_result(31, request.clone(), Some(found.clone()), Some(page(250))));
    assert!(state.accepts_find_reply(31, &request));
    assert!(state.set_filtered_find_result(
        31,
        request,
        Some(found),
        Some(publication(1, vec![200, 250], false)),
    ));
    assert_eq!(state.active_filter_target(), Some(target.clone()));
    assert!(state.table_filter_enabled());
    assert_eq!(state.table_selection.position(), Some((1, 1)));
    assert_eq!(state.data.as_ref().unwrap().parent_ordinal(1), Some(250));
    let occurrence = state
        .current_find_occurrence()
        .expect("current sparse Find");
    assert_eq!((occurrence.row, occurrence.column), (1, 1));
    assert_eq!((occurrence.start_utf16, occurrence.end_utf16), (0, 3));

    let request = state.current_find_request().unwrap();
    state.begin_find(32, request.clone());
    let mut wrong_mode = publication(1, vec![200, 250], false);
    wrong_mode.enabled = false;
    assert!(!state.set_filtered_find_result(
        32,
        request,
        Some(FindPosition {
            parent_ordinal: 250,
            column: "Name".into(),
            span: Some(FindSpan {
                start_utf16: 0,
                end_utf16: 3
            }),
        }),
        Some(wrong_mode)
    ));
    assert_eq!(state.active_filter_target(), Some(target.clone()));
    assert_eq!(state.table_selection.position(), Some((1, 1)));

    let request = state.current_find_request().unwrap();
    state.begin_find(33, request.clone());
    assert!(state.begin_filter_page(34, target, 0, true));
    assert!(!state.accepts_find_reply(33, &request));
}

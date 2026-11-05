use super::*;
use crate::{Action, Cell, Database, Table};
use egui::accesskit::Role;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};

fn state() -> State {
    let mut state = State::default();
    state.dismiss_backstage();
    state.find_available = true;
    state.database = Some(Database {
        document_revision: 7,
        title_path: None,
        title: "Find".into(),
        format: "ACE 16".into(),
        tables: vec![Table {
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

fn page(offset: u32) -> Data {
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
    oversized.find.as_mut().unwrap().pattern = "🦀".repeat(16_385);
    let request = oversized.current_find_request().unwrap();
    assert!(
        !oversized.can_find_next(&request),
        "UTF-8 byte limit is independent of editor scalar count"
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
    // Find's second combo is Match; changing this test to a non-popup button
    // would miss Escape ownership while its options are being inspected.
    harness
        .get_all_by_role(Role::ComboBox)
        .nth(1)
        .unwrap()
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

mod keyboard;

use crate::preview::{Preview, Scene};
use crate::{Action, ObjectKind, ObjectTarget, State};
use egui::{Key, accesskit::Role};
use egui_kittest::{Harness, kittest::Queryable};
use std::sync::Arc;

fn state() -> State {
    let mut preview = Preview::new(Scene::QuerySaveName);
    std::mem::take(preview.state_mut())
}

fn make_harness(state: State) -> Harness<'static, (State, Vec<Action>)> {
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1000.0, 700.0))
        .build_ui_state(
            |ui, state: &mut (State, Vec<Action>)| {
                state.1.extend(state.0.show(ui));
            },
            (state, Vec::new()),
        );
    crate::configure(&harness.ctx);
    harness.run_steps(2);
    harness
}

#[test]
fn name_dialog_requests_initial_focus_once_and_tab_keeps_the_ok_button_focused() {
    let mut harness = make_harness(state());
    assert!(
        harness
            .get_by_role_and_label(Role::TextInput, "Query Name")
            .is_focused()
    );
    harness.key_press(Key::Tab);
    harness.run_steps(3);
    assert!(
        harness
            .get_by_role_and_label(Role::Button, "OK")
            .is_focused()
    );
    assert!(harness.state().1.is_empty());
    assert!(harness.state().0.query_creation_name_open());
}

#[test]
fn enter_submits_current_typed_name_once_and_escape_emits_no_backend_action() {
    let mut harness = make_harness(state());
    harness
        .get_by_role_and_label(Role::TextInput, "Query Name")
        .focus();
    harness.step();
    harness.key_press_modifiers(egui::Modifiers::COMMAND, Key::A);
    harness.event(egui::Event::Paste("Typed query".into()));
    harness.run_steps(2);
    harness.key_press(Key::Enter);
    harness.run_steps(2);
    let [Action::ReviewQueryCreation { request }] = harness.state().1.as_slice() else {
        panic!("one explicit name submission expected");
    };
    assert_eq!(request.destination, "Typed query");
    assert_eq!(request.sql.as_str(), "SELECT 99 AS [Answer]");
    assert!(request.source.is_none());
    assert!(!harness.state().0.query_creation_name_open());

    let mut harness = make_harness(state());
    let sql = harness.state().0.sql.as_ptr();
    harness.key_press(Key::Escape);
    harness.run_steps(2);
    assert!(!harness.state().0.query_creation_name_open());
    assert_eq!(harness.state().0.sql.as_ptr(), sql);
    assert!(harness.state().1.is_empty());
}

#[test]
fn failure_keeps_current_destination_but_stale_reply_only_retires_its_issued_request() {
    let mut state = state();
    let Action::ReviewQueryCreation { request } = state.submit_query_creation_name().unwrap()
    else {
        panic!("name submission");
    };
    // NO_CLONE: The test models the host and UI retaining one immutable request.
    assert!(state.begin_query_creation_review(10, Arc::clone(&request)));
    assert!(state.fail_query_creation(10, &request, "unsupported source".into()));
    assert!(state.query_creation_name_open());
    let Action::ReviewQueryCreation { request: retry } =
        state.submit_query_creation_name().unwrap()
    else {
        panic!("retained destination submission");
    };
    assert_eq!(retry.destination, request.destination);
    // NO_CLONE: The UI association and delayed worker reply have independent owners.
    assert!(state.begin_query_creation_review(11, Arc::clone(&retry)));
    assert!(state.replace_query_sql("SELECT 101 AS [Answer]".into()));
    assert!(state.fail_query_creation(11, &retry, "stale response".into()));
    assert!(state.query_creation.pending.is_none());
    assert!(!state.query_creation_name_open());
    assert_eq!(state.sql, "SELECT 101 AS [Answer]");
    assert!(!state.fail_query_creation(10, &request, "older response".into()));
}

#[test]
fn saved_design_parks_under_its_real_owner_without_overwriting_global_scratch_sql() {
    let mut preview = Preview::new(Scene::SavedQueryDesign);
    let state = preview.state_mut();
    let saved = state.active_object().unwrap();
    assert!(matches!(saved.target.kind, ObjectKind::SavedQuery { .. }));
    assert!(state.query_design_visible());
    assert_eq!(state.query_design_sql(), Some("SELECT 99 AS [Answer];"));
    assert!(state.replace_query_sql("SELECT 7 AS [Scratch]".into()));
    let scratch = state
        .prepare_object_open(ObjectTarget {
            document_revision: saved.target.document_revision,
            kind: ObjectKind::Sql,
        })
        .unwrap()
        .tab;
    assert_eq!(state.sql, "SELECT 7 AS [Scratch]");
    assert!(state.activate_object(&saved));
    assert!(state.query_design_visible());
    assert_eq!(state.query_design_request().unwrap().tab, saved);
    assert_eq!(state.query_design_sql(), Some("SELECT 99 AS [Answer];"));
    assert!(state.activate_object(&scratch));
    assert_eq!(state.query_design_sql(), Some("SELECT 7 AS [Scratch]"));
}

#[test]
fn saved_design_capture_retains_its_sql_without_granting_a_scratch_run() {
    let mut preview = Preview::new(Scene::SavedQueryDesign);
    let state = preview.state_mut();
    assert!(state.replace_query_sql("SELECT 7 AS [Scratch]".into()));
    let captured = state.capture_query_draft().unwrap();
    assert_eq!(captured, "SELECT 99 AS [Answer];");
    assert_eq!(state.sql, "SELECT 7 AS [Scratch]");
    assert!(!state.begin_query_run(901, captured));
}

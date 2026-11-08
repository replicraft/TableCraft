use super::*;
use crate::{Database, ObjectTarget, SavedQuery, SavedQuerySqlEligibility, SavedQueryTarget};
use egui_kittest::Harness;

pub(in crate::workspace) fn inspected_state(edit_available: bool) -> (State, SavedQueryTarget) {
    let target = SavedQueryTarget {
        document_revision: 7,
        object_id: 90,
        name: "Saved query".into(),
    };
    let mut state = State {
        saved_query_sql_edit_available: edit_available,
        database: Some(Database {
            title_path: None,
            document_revision: 7,
            title: "SQL Save review".into(),
            format: "ACE 16".into(),
            tables: Vec::new(),
            queries: vec![SavedQuery {
                object_id: 90,
                name: target.name.clone(),
                system: false,
            }],
        }),
        ..State::default()
    };
    state.dismiss_backstage();
    state.begin_saved_query_inspection(target.clone(), 1);
    assert!(state.set_saved_query_details(1, target.clone(), Vec::new()));
    let request = state
        .begin_saved_query_sql_inspection(target.clone(), 2)
        .unwrap();
    assert!(state.set_saved_query_sql_inspection(
        2,
        &request,
        Some("SELECT 42 AS [Answer];".into()),
        SavedQuerySqlEligibility::QualifiedConstantSelect,
    ));
    (state, target)
}

pub(in crate::workspace) fn harness(state: State) -> Harness<'static, (State, Vec<Action>)> {
    Harness::builder()
        .with_size(egui::vec2(900.0, 650.0))
        .build_ui_state(
            |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
            (state, Vec::new()),
        )
}

#[test]
fn queued_save_captures_post_widget_text_once_with_full_identity_and_revision() {
    let (mut state, target) = inspected_state(true);
    assert!(!state.can_save_active_object());
    assert!(state.set_saved_query_sql_text("SELECT 43 AS [Answer];".into()));
    assert!(state.queue_active_save());
    assert!(state.queue_active_save());
    // Models an editor update after the QAT/shortcut queue in the same frame.
    let exact = "  SELECT 44 AS [Answer]; -- user formatting\n";
    assert!(state.set_saved_query_sql_text(exact.into()));
    let revision = state.saved_query_sql().unwrap().2;
    let mut actions = Vec::new();
    state.finish_active_save(&mut actions);
    state.finish_active_save(&mut actions);
    let [Action::ReviewSavedQuerySql { request }] = actions.as_slice() else {
        panic!("one source-bound SQL review request expected");
    };
    assert_eq!(request.target, target);
    assert_eq!(request.draft_revision, revision);
    assert_eq!(
        request.original_sql.as_deref(),
        Some("SELECT 42 AS [Answer];")
    );
    assert_eq!(request.proposed_sql, exact);
    assert!(state.saved_query_sql_request_current(request));
    assert!(!state.saved_query_sql_pending());
}

#[test]
fn queued_save_cannot_move_to_scratch_or_survive_a_busy_frame() {
    let (mut state, _) = inspected_state(true);
    assert!(state.set_saved_query_sql_text("SELECT 43 AS [Answer];".into()));
    assert!(state.queue_active_save());
    assert!(
        state
            .prepare_object_open(ObjectTarget {
                document_revision: 7,
                kind: ObjectKind::Sql,
            })
            .is_some()
    );
    let mut actions = Vec::new();
    state.finish_active_save(&mut actions);
    assert!(actions.is_empty());
    assert!(!state.can_save_active_object());

    let (mut state, _) = inspected_state(true);
    assert!(state.set_saved_query_sql_text("SELECT 43 AS [Answer];".into()));
    assert!(state.queue_active_save());
    state.busy = true;
    state.finish_active_save(&mut actions);
    state.busy = false;
    state.finish_active_save(&mut actions);
    assert!(actions.is_empty());
    assert!(state.saved_query_sql_dirty());
}

#[test]
fn pending_review_and_unavailable_host_cannot_queue_or_dispatch_save() {
    let (mut state, _) = inspected_state(false);
    assert!(!state.can_save_active_object());
    assert!(!state.set_saved_query_sql_text("SELECT 43 AS [Answer];".into()));
    assert!(!state.queue_active_save());
    let (mut state, _) = inspected_state(true);
    assert!(state.set_saved_query_sql_text("SELECT 43 AS [Answer];".into()));
    state.begin_saved_query_sql_review(99).unwrap();
    assert!(!state.queue_active_save());
    let mut actions = Vec::new();
    state.finish_active_save(&mut actions);
    assert!(actions.is_empty());
}

#[test]
fn central_save_preserves_table_draft_naming_workflow() {
    let mut state = State {
        table_save_available: true,
        database: Some(Database {
            title_path: None,
            document_revision: 7,
            title: "Table Save".into(),
            format: "ACE 16".into(),
            tables: Vec::new(),
            queries: Vec::new(),
        }),
        ..State::default()
    };
    state.dismiss_backstage();
    assert!(state.open_initial_table_draft());
    assert!(state.add_table_draft_field(tablecraft_builders::table::DraftFieldType::ShortText));
    assert!(state.can_save_active_object());
    assert!(state.queue_active_save());
    let mut harness = harness(state);
    harness.run_steps(2);
    assert!(harness.state().0.table_draft_save_open());
    assert!(
        harness
            .state()
            .1
            .iter()
            .all(|action| !matches!(action, Action::ReviewSavedQuerySql { .. }))
    );
}

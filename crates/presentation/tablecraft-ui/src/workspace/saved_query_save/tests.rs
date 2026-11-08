use super::*;
use crate::workspace::active_save::tests::{harness, inspected_state};
use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;

fn accepted() -> (State, SavedQuerySqlRequest) {
    accepted_with(" select 43 as [Answer];\n")
}

fn accepted_with(sql: &str) -> (State, SavedQuerySqlRequest) {
    let (mut state, _) = inspected_state(true);
    assert!(state.set_saved_query_sql_text(sql.into()));
    let request = state.begin_saved_query_sql_review(101).unwrap();
    assert!(state.set_saved_query_sql_review(101, &request, 202));
    (state, request)
}

fn saving() -> (State, SavedQuerySqlSaveIdentity) {
    let (mut state, request) = accepted();
    let prepared = state
        .prepare_saved_query_sql_save(101, &request, 202)
        .unwrap();
    assert_eq!(
        prepared.operation,
        SavedQuerySqlSaveOperation::PreparedReview
    );
    let identity = state.bind_saved_query_sql_save(prepared, 303).unwrap();
    (state, identity)
}

#[test]
fn preparation_owns_the_exact_review_before_binding_a_real_actor_id() {
    let (mut state, mut request) = accepted();
    request.target.name.push('x');
    assert!(
        state
            .prepare_saved_query_sql_save(101, &request, 202)
            .is_none()
    );
    assert_eq!(request.target.name.pop(), Some('x'));
    assert!(
        state
            .prepare_saved_query_sql_save(101, &request, 999)
            .is_none()
    );
    let prepared = state
        .prepare_saved_query_sql_save(101, &request, 202)
        .unwrap();
    assert_eq!(state.saved_query_sql_save_request(prepared), Some(&request));
    assert!(
        state
            .request_saved_query_sql_save_command(prepared, SavedQuerySqlSaveCommand::Cancel)
            .is_none()
    );
    let bytes = state.object_bytes().unwrap();
    let identity = state.bind_saved_query_sql_save(prepared, 303).unwrap();
    assert_eq!(
        state.object_bytes().unwrap(),
        bytes,
        "binding allocates no payload"
    );
    assert!(!state.abort_saved_query_sql_save(prepared));
    assert!(!state.clear_saved_query_sql_save(identity));
    assert!(
        state
            .prepare_saved_query_sql_save(101, &request, 202)
            .is_none()
    );
}

#[test]
fn failed_actor_start_and_failed_preadmission_preserve_the_original_dirty_review() {
    let (mut state, request) = accepted();
    let prepared = state
        .prepare_saved_query_sql_save(101, &request, 202)
        .unwrap();
    assert!(state.abort_saved_query_sql_save(prepared));
    assert!(!state.saved_query_sql_save_open());
    assert!(state.matches_saved_query_sql_review(101, &request, 202));
    state.sql = String::with_capacity(crate::workspace::objects::MAX_RETAINED_BYTES);
    assert!(
        state
            .prepare_saved_query_sql_save(101, &request, 202)
            .is_none()
    );
    assert!(!state.saved_query_sql_save_open());
    assert!(state.saved_query_sql_dirty());
}

#[test]
fn retained_native_resolution_changes_only_the_accepted_actor_id_and_rejects_stale_events() {
    let (mut state, identity) = saving();
    assert!(state.set_saved_query_sql_save_outcome(
        identity,
        EditSaveOutcome::Retained {
            message: "Native resources retained".into(),
            can_recover: true,
            can_release: false,
        }
    ));
    assert!(state.can_resolve_saved_query_sql_save(identity, true));
    assert!(!state.can_resolve_saved_query_sql_save(identity, false));
    assert!(!state.can_acknowledge_saved_query_sql_save(identity));
    assert!(!state.clear_saved_query_sql_save(identity));
    let next = state
        .begin_saved_query_sql_save_resolution(identity, 404)
        .unwrap();
    assert_eq!(next.request_id, identity.request_id);
    assert_eq!(next.review_id, identity.review_id);
    assert_eq!(
        next.operation,
        SavedQuerySqlSaveOperation::NativeSave { save_id: 404 }
    );
    assert!(
        !state.set_saved_query_sql_save_outcome(identity, EditSaveOutcome::Saved("stale".into()))
    );
    assert!(
        state
            .request_saved_query_sql_save_command(identity, SavedQuerySqlSaveCommand::Cancel)
            .is_none()
    );
    assert!(state.set_saved_query_sql_save_outcome(
        next,
        EditSaveOutcome::Cancelled("Original restored".into())
    ));
    assert!(
        state
            .request_saved_query_sql_save_command(next, SavedQuerySqlSaveCommand::Acknowledge)
            .is_some()
    );
    assert!(
        state.saved_query_sql_save_open(),
        "acknowledgement intent does not clear ownership"
    );
    assert!(state.clear_saved_query_sql_save(next));
    assert!(state.saved_query_sql_dirty());
}

#[test]
fn native_terminal_fact_and_full_text_survive_view_publication_and_failed_read_reopen() {
    let (mut state, identity) = saving();
    let mut report = String::with_capacity(32 * 1024);
    report.push_str("Known replacement persisted");
    let capacity = report.capacity();
    let before = state.object_bytes().unwrap();
    assert!(state.set_saved_query_sql_save_outcome(identity, EditSaveOutcome::Saved(report)));
    assert!(state.object_bytes().unwrap() >= before + capacity);
    state.database.as_mut().unwrap().document_revision += 1;
    state.reset_saved_query();
    assert_eq!(
        state
            .saved_query_sql_save_request(identity)
            .unwrap()
            .proposed_sql,
        " select 43 as [Answer];\n"
    );
    assert!(state.begin_saved_query_sql_reopen(identity));
    assert!(state.set_saved_query_sql_reopen_failed(identity, "Read witness changed".into()));
    assert!(
        matches!(state.saved_query_sql_save_status(), Some((key, Some(EditSaveOutcome::Saved(_)))) if key == identity)
    );
    assert!(
        state
            .request_saved_query_sql_save_command(identity, SavedQuerySqlSaveCommand::RetryReopen)
            .is_some()
    );
    assert!(state.saved_query_sql_save_open());
    assert!(state.begin_saved_query_sql_reopen(identity));
    assert!(state.clear_saved_query_sql_save(identity));
}

#[test]
fn unchanged_review_has_no_native_save_id_receipt_or_recovery_commands() {
    let (mut state, request) = accepted_with(" select 42 as [Answer];\n");
    let identity = state
        .begin_saved_query_sql_unchanged_reopen(101, &request, 202)
        .unwrap();
    assert_eq!(
        identity.operation,
        SavedQuerySqlSaveOperation::UnchangedReview
    );
    assert!(state.saved_query_sql_save_status().unwrap().1.is_none());
    assert!(
        !state.set_saved_query_sql_save_outcome(
            identity,
            EditSaveOutcome::Saved("fabricated".into())
        )
    );
    assert!(!state.can_resolve_saved_query_sql_save(identity, true));
    assert!(!state.can_acknowledge_saved_query_sql_save(identity));
    assert!(state.set_saved_query_sql_reopen_failed(identity, "Read-only reopen refused".into()));
    for command in [
        SavedQuerySqlSaveCommand::Cancel,
        SavedQuerySqlSaveCommand::Recover,
        SavedQuerySqlSaveCommand::Release,
        SavedQuerySqlSaveCommand::Acknowledge,
    ] {
        assert!(
            state
                .request_saved_query_sql_save_command(identity, command)
                .is_none()
        );
    }
    assert!(
        state
            .request_saved_query_sql_save_command(identity, SavedQuerySqlSaveCommand::ReturnToDraft)
            .is_some()
    );
    assert!(state.saved_query_sql_dirty());
}

#[test]
fn native_status_dialog_emits_exact_cancel_identity_without_a_cell_edit_dialog() {
    let (state, identity) = saving();
    let mut harness = harness(state);
    harness.run_steps(2);
    assert!(!harness.state().0.edit_dialog_open());
    harness
        .get_by_role_and_label(Role::Button, "Request cancellation")
        .click_accesskit();
    harness.run_steps(1);
    assert!(
        matches!(harness.state().1.as_slice(), [Action::SavedQuerySqlSave { identity: actual, command: SavedQuerySqlSaveCommand::Cancel }] if *actual == identity)
    );
    assert!(harness.state().0.saved_query_sql_save_open());
}

#[test]
fn unchanged_reopen_failure_dialog_exposes_only_read_retry_or_return() {
    let (mut state, request) = accepted_with(" select 42 as [Answer];\n");
    let identity = state
        .begin_saved_query_sql_unchanged_reopen(101, &request, 202)
        .unwrap();
    assert!(state.set_saved_query_sql_reopen_failed(identity, "Read-only reopen refused".into()));
    let mut harness = harness(state);
    harness.run_steps(2);
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Recover save")
            .is_none()
    );
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Release save resources")
            .is_none()
    );
    harness
        .get_by_role_and_label(Role::Button, "Retry reopening")
        .click_accesskit();
    harness.run_steps(1);
    assert!(
        matches!(harness.state().1.as_slice(), [Action::SavedQuerySqlSave { identity: actual, command: SavedQuerySqlSaveCommand::RetryReopen }] if *actual == identity)
    );
    assert!(harness.state().0.saved_query_sql_save_open());
}

#[test]
fn unchanged_read_reopen_escape_requests_cancellation_without_native_save_identity() {
    let (mut state, request) = accepted_with(" select 42 as [Answer];\n");
    let identity = state
        .begin_saved_query_sql_unchanged_reopen(101, &request, 202)
        .unwrap();
    let mut harness = harness(state);
    harness.run_steps(2);
    harness.get_by_role_and_label(Role::Button, "Request cancellation");
    harness.key_press(egui::Key::Escape);
    harness.run_steps(1);
    assert!(
        matches!(harness.state().1.as_slice(), [Action::SavedQuerySqlSave { identity: actual, command: SavedQuerySqlSaveCommand::Cancel }] if *actual == identity)
    );
    assert_eq!(
        identity.operation,
        SavedQuerySqlSaveOperation::UnchangedReview
    );
    harness.run_steps(1);
    harness.get_by_label("Cancellation requested. Waiting for the database read to stop.");

    assert!(
        harness
            .state()
            .0
            .saved_query_sql_save_status()
            .unwrap()
            .1
            .is_none()
    );
    assert!(
        harness
            .state_mut()
            .0
            .request_saved_query_sql_save_command(identity, SavedQuerySqlSaveCommand::Cancel)
            .is_none()
    );
    assert!(
        harness
            .state_mut()
            .0
            .set_saved_query_sql_reopen_failed(identity, "Read cancelled".into())
    );
    assert_eq!(
        harness.state().0.saved_query_sql_reopen_error(identity),
        Some("Read cancelled")
    );
    assert!(
        harness
            .state()
            .0
            .can_return_to_saved_query_sql_draft(identity)
    );
    assert!(harness.state().0.saved_query_sql_dirty());
}

#[test]
fn terminal_and_read_failure_affordances_survive_report_admission_failure() {
    let (mut state, identity) = saving();
    state.sql = String::with_capacity(crate::workspace::objects::MAX_RETAINED_BYTES);
    assert!(state.set_saved_query_sql_save_outcome(
        identity,
        EditSaveOutcome::Retained {
            message: "Native recovery owner must stay accessible".into(),
            can_recover: true,
            can_release: true,
        }
    ));
    assert!(state.can_resolve_saved_query_sql_save(identity, true));
    assert!(state.can_resolve_saved_query_sql_save(identity, false));
    assert!(
        matches!(state.saved_query_sql_save_status(), Some((_, Some(EditSaveOutcome::Retained { message, .. }))) if message.is_empty())
    );
    let identity = state
        .begin_saved_query_sql_save_resolution(identity, 404)
        .unwrap();
    assert!(state.set_saved_query_sql_save_outcome(
        identity,
        EditSaveOutcome::Saved("Known persisted replacement".into())
    ));
    assert!(state.can_acknowledge_saved_query_sql_save(identity));
    assert!(state.begin_saved_query_sql_reopen(identity));
    assert!(state.set_saved_query_sql_reopen_failed(identity, "Read witness refused".into()));
    assert!(state.can_return_to_saved_query_sql_draft(identity));
    assert_eq!(state.saved_query_sql_reopen_error(identity), Some(""));
    assert!(
        matches!(state.saved_query_sql_save_status(), Some((_, Some(EditSaveOutcome::Saved(message)))) if message.is_empty())
    );
    assert!(state.saved_query_sql_save.as_ref().unwrap().report_omitted);
    assert!(
        state
            .saved_query_sql_save
            .as_ref()
            .unwrap()
            .reopen_error_omitted
    );
}

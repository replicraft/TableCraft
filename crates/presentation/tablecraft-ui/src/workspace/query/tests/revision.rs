//! Prompt witnesses track edits separately from submitted execution results.
use super::{harness, result, state};
use crate::workspace::{Action, query::provenance::ResultOrigin};
use egui::{Key, Modifiers, accesskit::Role};
use egui_kittest::kittest::{NodeT, Queryable};

#[test]
fn actual_sql_editor_aba_invalidates_inspection_without_discarding_prior_results() {
    let mut state = state();
    state.sql = "SELECT 1".into();
    assert!(state.set_query_data(Some("SELECT 1".into()), result("original")));
    let revision = state.query_sql_revision().unwrap();
    let rows = state.query_data.as_ref().unwrap().rows.as_ptr();
    let mut harness = harness(state);
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::MultilineTextInput, "SQL query")
        .focus();
    harness.run_steps(1);
    for sql in ["SELECT 2", "SELECT 1"] {
        harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
        harness
            .get_by_role_and_label(Role::MultilineTextInput, "SQL query")
            .type_text(sql);
        harness.run_steps(1);
    }
    let state = &harness.state().0;
    assert_eq!(state.sql, "SELECT 1");
    assert!(state.query_sql_revision().unwrap() > revision);
    assert!(!state.query_sql_witness_current(revision, "SELECT 1"));
    assert_eq!(state.query_data.as_ref().unwrap().rows.as_ptr(), rows);
    assert!(matches!(&state.query_result, ResultOrigin::Sql(sql) if sql == "SELECT 1"));
    assert!(harness.state().1.is_empty());
}

#[test]
fn replacement_revision_and_exact_text_guards_keep_executed_results_separate() {
    let mut state = state();
    state.sql = "SELECT 1".into();
    let original_revision = state.query_sql_revision().unwrap();
    assert!(state.query_sql_witness_current(original_revision, "SELECT 1"));
    assert!(state.begin_query_run(1, "SELECT 1".into()));
    assert!(state.replace_query_sql("SELECT 2".into()));
    assert!(state.replace_query_sql("SELECT 1".into()));
    assert!(!state.query_sql_witness_current(original_revision, "SELECT 1"));
    let current_revision = state.query_sql_revision().unwrap();
    assert!(state.replace_query_sql("SELECT 1".into()));
    assert_eq!(state.query_sql_revision(), Some(current_revision));

    // Existing hosts can assign the public draft directly. Exact text must
    // still guard a prompt, even when that caller did not advance the clock.
    state.sql = "SELECT 3".into();
    assert!(!state.query_sql_witness_current(current_revision, "SELECT 1"));
    assert!(state.set_query_run_data(1, result("executed SELECT 1")));
    assert!(state.query_results_previous());
    assert!(matches!(&state.query_result, ResultOrigin::Sql(sql) if sql == "SELECT 1"));
    state.database.as_mut().unwrap().document_revision += 1;
    assert!(!state.query_sql_witness_current(current_revision, "SELECT 3"));
}

#[test]
fn same_frame_typing_and_run_capture_the_latest_text_revision() {
    let mut state = state();
    state.sql = "SELECT 1".into();
    let revision = state.query_sql_revision().unwrap();
    let mut harness = harness(state);
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::MultilineTextInput, "SQL query")
        .focus();
    harness.run_steps(1);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    harness
        .get_by_role_and_label(Role::MultilineTextInput, "SQL query")
        .type_text("SELECT 2");
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Enter);
    harness.run_steps(1);
    assert!(matches!(harness.state().1.as_slice(), [Action::Query]));
    let state = &mut harness.state_mut().0;
    let captured = state.capture_query_draft().unwrap();
    let latest = state.query_sql_revision().unwrap();
    assert_eq!(captured, "SELECT 2");
    assert!(latest > revision);
    assert!(state.query_sql_witness_current(latest, &captured));
}

#[test]
fn revision_exhaustion_refuses_new_admission_and_keeps_text_results_and_pending_owner() {
    let mut state = state();
    state.sql = "SELECT 1".into();
    assert!(state.set_query_data(Some("SELECT 1".into()), result("original")));
    assert!(state.begin_query_run(1, "SELECT 1".into()));
    let rows = state.query_data.as_ref().unwrap().rows.as_ptr();
    state.query_sql_revision = u64::MAX;
    assert_eq!(state.query_sql_revision(), None);
    assert!(!state.query_sql_witness_current(u64::MAX, "SELECT 1"));
    assert!(!state.replace_query_sql("SELECT 2".into()));
    assert!(!state.record_query_sql_edit());
    assert!(state.capture_query_draft().is_none());
    assert!(!state.begin_query_run(2, "SELECT 2".into()));
    assert_eq!(state.sql, "SELECT 1");
    assert_eq!(state.query_sql_revision, u64::MAX);
    assert_eq!(state.query_data.as_ref().unwrap().rows.as_ptr(), rows);
    assert_eq!(state.query_pending.as_ref().unwrap().sql, "SELECT 1");

    let mut harness = harness(state);
    harness.run_steps(2);
    assert!(
        harness
            .get_by_role_and_label(Role::MultilineTextInput, "SQL query")
            .accesskit_node()
            .is_disabled()
    );
    assert_eq!(harness.state().0.sql, "SELECT 1");
    assert!(harness.state().1.is_empty());
}

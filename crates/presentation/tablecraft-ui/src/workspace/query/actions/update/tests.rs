use super::*;
use crate::workspace::{
    Action,
    query::tests::{harness as make_harness, result, state},
};
use egui::{Key, accesskit::Role};
use egui_kittest::kittest::Queryable;

const INSPECTION: u64 = 11;
const REQUEST: u64 = 12;
const REVIEW: u64 = 13;
const SQL: &str = "UPDATE Items SET Score=77 WHERE Id>=2";

fn reviewing() -> State {
    let mut state = state();
    assert!(state.replace_query_sql(SQL.into()));
    assert!(state.set_query_data(Some("SELECT 1".into()), result("old result")));
    let captured = state.capture_query_draft().unwrap();
    let original_allocation = captured.as_ptr();
    assert!(state.begin_query_parameter_inspection(INSPECTION, captured));
    assert!(matches!(
        state.set_query_parameter_details(INSPECTION, SQL, Vec::new()),
        Some(Action::RunParameterizedQuery { inspection_id: INSPECTION, parameters })
            if parameters.is_empty()
    ));
    assert!(state.begin_query_update_review(INSPECTION, SQL, REQUEST, Vec::new()));
    assert_eq!(
        state.query_update.as_ref().unwrap().origin.sql.as_ptr(),
        original_allocation
    );
    state
}

fn facts() -> QueryUpdateReview {
    QueryUpdateReview {
        id: REVIEW,
        document_revision: 7,
        table: "Items".into(),
        matched_rows: 4,
        changed_rows: 2,
    }
}

fn reviewed() -> State {
    let mut state = reviewing();
    assert!(state.set_query_update_review(REQUEST, facts()));
    state
}

#[test]
fn actual_yes_and_no_preserve_results_and_use_exact_review_id() {
    for yes in [false, true] {
        let state = reviewed();
        let rows = state.query_data.as_ref().unwrap().rows.as_ptr();
        let selected = state.query_selection.position();
        let mut harness = make_harness(state);
        harness.run_steps(2);
        for (key, count) in [("query-update-matched", "4"), ("query-update-changed", "2")] {
            let mut arguments = crate::FluentArgs::new();
            arguments.set("count", count);
            let label = harness.state().0.localizer.format(key, &arguments);
            assert!(harness.query_by_label(&label).is_some());
        }
        assert!(
            harness
                .get_by_role_and_label(Role::Button, "No")
                .is_focused()
        );
        harness
            .get_by_role_and_label(Role::Button, if yes { "Yes" } else { "No" })
            .click_accesskit();
        harness.run_steps(2);
        assert_eq!(
            harness.state().0.query_data.as_ref().unwrap().rows.as_ptr(),
            rows
        );
        assert_eq!(harness.state().0.query_selection.position(), selected);
        assert_eq!(harness.state().0.sql, SQL);
        if yes {
            assert!(matches!(
                harness.state().1.as_slice(),
                [Action::ApplyQueryUpdate { review_id: REVIEW }]
            ));
            assert!(harness.state().0.can_apply_query_update(REVIEW));
            assert!(!harness.state().0.can_apply_query_update(REVIEW + 1));
        } else {
            assert!(matches!(
                harness.state().1.as_slice(),
                [Action::CancelQueryUpdateReview {
                    request_id: REQUEST
                }]
            ));
            assert!(!harness.state().0.query_update_dialog_open());
        }
    }
}

#[test]
fn escape_cancels_review_but_after_acceptance_only_requests_cancellation() {
    let mut harness = make_harness(reviewed());
    harness.run_steps(2);
    harness.key_press(Key::Escape);
    harness.run_steps(2);
    assert!(!harness.state().0.query_update_dialog_open());
    assert!(matches!(
        harness.state().1.as_slice(),
        [Action::CancelQueryUpdateReview {
            request_id: REQUEST
        }]
    ));

    let mut state = reviewed();
    assert!(state.begin_query_update_save(REVIEW, 71));
    assert!(!state.close_query_update());
    let mut harness = make_harness(state);
    harness.run_steps(2);
    harness.key_press(Key::Escape);
    harness.run_steps(2);
    assert!(harness.state().0.query_update_save_pending());
    assert!(matches!(
        harness.state().1.as_slice(),
        [Action::CancelEditSave { save_id: 71 }]
    ));
    assert!(matches!(
        harness.state().0.query_update.as_ref().unwrap().phase,
        Phase::Saving {
            cancel_requested: true,
            ..
        }
    ));
}

#[test]
fn accepted_save_survives_origin_changes_and_requires_matching_terminal_acknowledgment() {
    let mut state = reviewed();
    assert!(state.begin_query_update_save(REVIEW, 71));
    assert!(state.replace_query_sql("SELECT 2".into()));
    state.bump_focus_publication();
    state.validate_query_update();
    assert_eq!(state.query_update_save_id(), Some(71));
    assert!(!state.set_query_update_save_outcome(99, EditSaveOutcome::Saved("wrong".into())));
    assert!(state.set_query_update_save_outcome(71, EditSaveOutcome::Saved("Saved".into())));
    assert!(!state.close_query_update());
    assert!(!state.acknowledge_query_update_save(99));
    assert!(state.can_acknowledge_query_update_save(71));
    assert!(state.acknowledge_query_update_save(71));
    assert!(!state.query_update_dialog_open());
}

#[test]
fn retained_save_keeps_exact_recovery_permissions_and_terminal_owner() {
    let mut state = reviewed();
    assert!(state.begin_query_update_save(REVIEW, 71));
    assert!(state.set_query_update_save_outcome(
        71,
        EditSaveOutcome::Retained {
            message: "Retained resources".into(),
            can_recover: false,
            can_release: true,
        }
    ));
    assert!(!state.can_resolve_query_update_save(71, true));
    assert!(state.can_resolve_query_update_save(71, false));
    assert!(!state.can_resolve_query_update_save(99, false));
    assert!(!state.acknowledge_query_update_save(71));
    assert!(!state.close_query_update());
    assert!(state.begin_query_update_resolution(71, 72));
    assert!(
        !state.set_query_update_save_outcome(71, EditSaveOutcome::Failed("late report".into()))
    );
    assert!(state.set_query_update_save_outcome(72, EditSaveOutcome::Cancelled("Released".into())));
    assert!(state.acknowledge_query_update_save(72));
}

#[test]
fn stale_sql_aba_document_and_invalid_review_counts_never_enable_yes() {
    for change in 0..3 {
        let mut state = reviewed();
        match change {
            0 => {
                assert!(state.replace_query_sql("SELECT 2".into()));
                assert!(state.replace_query_sql(SQL.into()));
            }
            1 => state.database.as_mut().unwrap().document_revision += 1,
            2 => state.bump_focus_publication(),
            _ => unreachable!(),
        }
        assert!(!state.can_apply_query_update(REVIEW));
        state.validate_query_update();
        assert!(!state.query_update_dialog_open());
    }
    let mut state = reviewing();
    assert!(!state.set_query_update_review(REQUEST + 1, facts()));
    let mut invalid = facts();
    invalid.changed_rows = invalid.matched_rows + 1;
    assert!(!state.set_query_update_review(REQUEST, invalid));
    assert!(!state.can_apply_query_update(REVIEW));
    assert!(state.query_update_review_current(REQUEST, SQL));
}

#[test]
fn refused_diagnostic_capacity_never_discards_terminal_resource_permissions() {
    let mut state = reviewing();
    let before = state.object_bytes().unwrap();
    let mut message = String::with_capacity(MAX_REPORT_BYTES + 1);
    message.push_str("short text with excessive capacity");
    state.fail_query_update_review(REQUEST, message);
    assert_eq!(state.object_bytes(), Some(before));
    assert!(matches!(
        &state.query_update.as_ref().unwrap().phase,
        Phase::Reviewing {
            error: Some(Report::Limit),
            ..
        }
    ));
    assert!(state.set_query_update_review(REQUEST, facts()));
    let before = state.object_bytes().unwrap();
    state.fail_query_update_apply(REVIEW, "x".repeat(MAX_REPORT_BYTES + 1));
    assert_eq!(state.object_bytes(), Some(before));
    assert!(state.can_apply_query_update(REVIEW));
    assert!(state.begin_query_update_save(REVIEW, 71));
    assert!(state.set_query_update_save_outcome(
        71,
        EditSaveOutcome::Retained {
            message: "x".repeat(MAX_REPORT_BYTES + 1),
            can_recover: false,
            can_release: true,
        }
    ));
    assert!(matches!(&state.query_update.as_ref().unwrap().phase,
        Phase::Finished { report_refused: true, outcome: EditSaveOutcome::Retained { message, .. }, .. } if message.capacity() == 0));
    assert!(!state.can_resolve_query_update_save(71, true));
    assert!(state.can_resolve_query_update_save(71, false));
    assert!(!state.close_query_update());
    assert!(!state.acknowledge_query_update_save(71));
    let mut harness = make_harness(state);
    harness.run_steps(2);
    let diagnostic = harness.state().0.localizer.text("objects-memory-limit");
    assert!(harness.query_by_label(&diagnostic).is_some());
    let release = harness.state().0.localizer.text("editing-release");
    assert!(
        harness
            .query_by_role_and_label(Role::Button, &release)
            .is_some()
    );
}

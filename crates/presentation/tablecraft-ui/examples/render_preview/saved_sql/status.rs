//! Typed status illustrations only: no runtime, actor, journal or native witness.
use super::{LAYOUT_SETTLE_STEPS, assert_painted_in};
use egui::{Id, accesskit::Role};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use tablecraft_ui::{
    EditSaveOutcome, SavedQuerySqlRequest, SavedQuerySqlSaveCommand, SavedQuerySqlSaveIdentity,
    SavedQuerySqlSaveOperation, preview::Preview,
};

const REVIEW_REQUEST_ID: u64 = 720;
const REVIEW_ID: u64 = 721;
// An example-only presentation ID. No SaveWorker is created or started here.
const PRESENTATION_SAVE_ID: u64 = 722;
const UNCHANGED: &str = "select 42 as [Answer];\n";

pub(super) fn is_mode(mode: &str) -> bool {
    matches!(
        mode,
        "save-pending"
            | "save-terminal"
            | "save-retained"
            | "save-diagnosis"
            | "save-reopen-failed"
            | "unchanged-reopening"
            | "unchanged-reopen-failed"
            | "unchanged-cancelled"
    )
}

pub(super) fn prepare(harness: &mut Harness<'_, Preview>, mode: &str) {
    let (identity, request) = {
        let state = harness.state_mut().state_mut();
        if mode.starts_with("unchanged-") {
            assert!(state.set_saved_query_sql_text(UNCHANGED.into()));
        }
        let request = state
            .begin_saved_query_sql_review(REVIEW_REQUEST_ID)
            .unwrap();
        assert!(state.set_saved_query_sql_review(REVIEW_REQUEST_ID, &request, REVIEW_ID));
        let report = state.localizer.text("editing-mock-diagnosis");
        let identity = if mode.starts_with("unchanged-") {
            let identity = state
                .begin_saved_query_sql_unchanged_reopen(REVIEW_REQUEST_ID, &request, REVIEW_ID)
                .unwrap();
            if mode == "unchanged-reopen-failed" {
                assert!(state.set_saved_query_sql_reopen_failed(identity, report));
            } else if mode == "unchanged-cancelled" {
                assert!(
                    state
                        .request_saved_query_sql_save_command(
                            identity,
                            SavedQuerySqlSaveCommand::Cancel
                        )
                        .is_some()
                );
            }
            identity
        } else {
            let prepared = state
                .prepare_saved_query_sql_save(REVIEW_REQUEST_ID, &request, REVIEW_ID)
                .unwrap();
            let identity = state
                .bind_saved_query_sql_save(prepared, PRESENTATION_SAVE_ID)
                .unwrap();
            match mode {
                "save-retained" | "save-diagnosis" => {
                    assert!(state.set_saved_query_sql_save_outcome(
                        identity,
                        EditSaveOutcome::Retained {
                            message: report,
                            can_recover: mode == "save-retained",
                            can_release: mode == "save-retained",
                        }
                    ));
                }
                "save-terminal" => {
                    assert!(state.set_saved_query_sql_save_outcome(
                        identity,
                        EditSaveOutcome::Saved(report)
                    ));
                }
                "save-reopen-failed" => {
                    // NO_CLONE: Actual source-free saved report and subsequent read failure
                    // warning coexist in distinct retained status owners; sharing mutable
                    // String storage would change ownership.
                    assert!(state.set_saved_query_sql_save_outcome(
                        identity,
                        EditSaveOutcome::Saved(report.clone())
                    ));
                    assert!(state.begin_saved_query_sql_reopen(identity));
                    assert!(state.set_saved_query_sql_reopen_failed(identity, report));
                }
                "save-pending" => {}
                _ => unreachable!("validated status preview mode"),
            }
            identity
        };
        // Stale or crossed request/review/actor signatures cannot replace the
        // seeded presentation outcome. These checks create no native authority.
        let mut stale = identity;
        stale.review_id += 1;
        assert!(
            !state.set_saved_query_sql_save_outcome(stale, EditSaveOutcome::Failed(String::new()))
        );
        assert!(
            state
                .request_saved_query_sql_save_command(stale, SavedQuerySqlSaveCommand::Cancel)
                .is_none()
        );
        if let SavedQuerySqlSaveOperation::NativeSave { save_id } = identity.operation {
            let mut stale_actor = identity;
            stale_actor.operation = SavedQuerySqlSaveOperation::NativeSave {
                save_id: save_id + 1,
            };
            assert!(!state.set_saved_query_sql_save_outcome(
                stale_actor,
                EditSaveOutcome::Failed(String::new())
            ));
        }
        assert_eq!(state.saved_query_sql_save_request(identity), Some(&request));
        (identity, request)
    };
    // Pending spinners request continuous repaints. Advance a fixed number of
    // frames rather than waiting for a resource-free animated state to idle.
    harness.run_steps(LAYOUT_SETTLE_STEPS + 1);
    harness.remove_cursor();
    assert_status(harness, mode, identity, &request);
}

fn assert_status(
    harness: &Harness<'_, Preview>,
    mode: &str,
    identity: SavedQuerySqlSaveIdentity,
    request: &SavedQuerySqlRequest,
) {
    let state = harness.state().state();
    assert!(state.saved_query_sql_save_open());
    assert!(!state.edit_dialog_open());
    assert_eq!(state.saved_query_sql_save_request(identity), Some(request));
    assert_eq!(identity.request_id, REVIEW_REQUEST_ID);
    assert_eq!(identity.review_id, REVIEW_ID);
    assert_eq!(request.original_sql.as_deref(), Some(super::ORIGINAL));
    assert_eq!(state.selected_saved_target(), Some(&request.target));
    assert_eq!(
        state.saved_query_sql().map(|(_, _, revision, _)| revision),
        Some(request.draft_revision)
    );
    let modal = harness
        .ctx
        .memory(|memory| memory.area_rect(Id::new("saved_query_sql_save_status")))
        .expect("the dedicated query status modal owns its retained area");
    assert!(harness.ctx.viewport_rect().contains_rect(modal));
    assert_painted_in(harness, &state.localizer.text("query-title"), modal);
    assert_painted_in(harness, &request.target.name, modal);
    for (key, expected) in [
        (
            "editing-request-cancellation",
            matches!(mode, "save-pending" | "unchanged-reopening"),
        ),
        ("editing-recover", mode == "save-retained"),
        ("editing-release", mode == "save-retained"),
        ("backstage-close", mode == "save-terminal"),
        (
            "saved-query-save-retry-reopen",
            mode.ends_with("reopen-failed"),
        ),
        (
            "saved-query-save-return-draft",
            mode.ends_with("reopen-failed"),
        ),
    ] {
        let label = state.localizer.text(key);
        let buttons = harness
            .query_all_by_role_and_label(Role::Button, &label)
            .filter(|button| modal.contains_rect(button.rect()))
            .collect::<Vec<_>>();
        assert_eq!(buttons.len(), usize::from(expected), "{mode}: {key}");
        if let Some(button) = buttons.first() {
            assert!(!button.accesskit_node().is_disabled());
            assert_painted_in(harness, &label, modal);
        }
    }
    let label = state.localizer.text("query-title");
    assert!(
        harness
            .get_by_role_and_label(Role::MultilineTextInput, &label)
            .accesskit_node()
            .is_disabled()
    );
    assert_eq!(
        state.can_return_to_saved_query_sql_draft(identity),
        mode.ends_with("reopen-failed")
    );
    if mode.starts_with("unchanged-") {
        assert_eq!(
            identity.operation,
            SavedQuerySqlSaveOperation::UnchangedReview
        );
        assert!(state.saved_query_sql_save_status().unwrap().1.is_none());
        assert_eq!(request.proposed_sql, UNCHANGED);
        assert!(!state.can_resolve_saved_query_sql_save(identity, true));
        assert!(!state.can_resolve_saved_query_sql_save(identity, false));
        assert!(!state.can_acknowledge_saved_query_sql_save(identity));
    } else {
        assert_eq!(
            identity.operation,
            SavedQuerySqlSaveOperation::NativeSave {
                save_id: PRESENTATION_SAVE_ID
            }
        );
        assert_eq!(request.proposed_sql, super::PROPOSED);
    }
    assert_eq!(
        state.can_acknowledge_saved_query_sql_save(identity),
        mode == "save-terminal"
    );
    assert_eq!(
        state.can_resolve_saved_query_sql_save(identity, true),
        mode == "save-retained"
    );
    assert_eq!(
        state.can_resolve_saved_query_sql_save(identity, false),
        mode == "save-retained"
    );
    if mode == "save-diagnosis" {
        assert_painted_in(
            harness,
            &state.localizer.text("editing-diagnosis-required"),
            modal,
        );
    }
    if mode == "unchanged-cancelled" {
        assert_painted_in(
            harness,
            &state
                .localizer
                .text("saved-query-read-cancellation-requested"),
            modal,
        );
    }
    if mode.ends_with("reopen-failed") {
        assert!(state.saved_query_sql_reopen_error(identity).is_some());
    }
}

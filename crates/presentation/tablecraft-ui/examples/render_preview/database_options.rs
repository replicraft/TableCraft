//! Source-free Options presentation. No database, runtime review, native save,
//! resource lease or full-image witness exists in this renderer. The retained
//! Windows Access Account capture informs the File rail only: no Options,
//! Current Database, Name AutoCorrect or close-objects capture is retained.
use super::LAYOUT_SETTLE_STEPS;
use egui::{Id, Shape, accesskit::Role};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use tablecraft_ui::{EditSaveOutcome, NameAutoCorrectState, SaveCleanupNotice, preview::Preview};

const INSPECTION_ID: u64 = 930;
const REVIEW_REQUEST_ID: u64 = 931;
const REVIEW_ID: u64 = 932;
// Example-only presentation identity. No SaveWorker is constructed or started.
const PRESENTATION_SAVE_ID: u64 = 933;

pub(super) fn is_mode(mode: &str) -> bool {
    matches!(
        mode,
        "none" | "general" | "unknown" | "refused" | "close" | "review" | "returned"
    )
}

pub(super) fn prepare(harness: &mut Harness<'_, Preview>, mode: &str) {
    let old_tabs = harness.state().state().object_tabs();
    let revision = harness
        .state()
        .state()
        .database
        .as_ref()
        .expect("datasheet metadata")
        .document_revision;
    {
        let state = harness.state_mut().state_mut();
        state.show_options_dialog();
        if mode != "general" {
            assert!(state.begin_database_options_inspection(revision, INSPECTION_ID));
            match mode {
                "unknown" => {
                    // Illustrative pending read only. Never replace Unknown
                    // with an inferred stored value or native eligibility.
                    state.busy = true;
                }
                "refused" => {
                    let message = state.localizer.text("editing-mock-diagnosis");
                    assert!(state.fail_database_options_inspection(
                        revision,
                        INSPECTION_ID,
                        message
                    ));
                }
                _ => {
                    // Presentation fixture only; no native source was qualified.
                    state.database_options_available = true;
                    assert!(state.set_database_options_inspection(
                        revision,
                        INSPECTION_ID,
                        NameAutoCorrectState::Missing,
                        true,
                    ));
                }
            }
        }
    }
    settle(harness);
    if mode != "general" {
        let label = harness
            .state()
            .state()
            .localizer
            .text("database-options-current-database");
        harness.get_by_label(&label).click();
        settle(harness);
        // Verify the actual category interaction before any direct illustrative
        // state setup. A foreground close modal must not conceal a missed click
        // that left the General pane in the background.
        let modal = area(harness, "application_options");
        let state_key = if matches!(mode, "unknown" | "refused") {
            "database-options-state-unknown"
        } else {
            "database-options-state-missing"
        };
        painted(harness, state_key, modal);
        button(harness, "backstage-close", false, modal);
    }
    if matches!(mode, "close" | "review" | "returned") {
        let state = harness.state_mut().state_mut();
        assert!(state.request_database_options_close_confirmation());
        if mode != "close" {
            let request = state
                .approve_database_options_close()
                .expect("exact UI-only graph confirmation");
            assert!(state.database_options_close_request_current(request));
            assert!(state.begin_database_options_review(revision, REVIEW_REQUEST_ID));
            if mode == "review" {
                state.busy = true;
            } else {
                let identity = state
                    .prepare_database_options_save(revision, REVIEW_REQUEST_ID, REVIEW_ID)
                    .expect("presentation reservation only")
                    .bind(PRESENTATION_SAVE_ID);
                let report = state.localizer.text("editing-mock-diagnosis");
                assert!(state.set_database_options_save_outcome(
                    identity,
                    EditSaveOutcome::Saved(report.clone())
                ));
                assert!(state.begin_database_options_reopen(identity));
                assert!(state.fail_database_options_reopen(identity, report));
                assert!(state.return_from_database_options_reopen(
                    identity,
                    Some(SaveCleanupNotice::RetainedPrivateJournal)
                ));
                state.database_options_restore_settings();
                assert!(state.native_source_stale());
                assert!(state.native_mutation_blocked());
            }
        }
        settle(harness);
    }
    assert_eq!(
        harness.state().state().object_tabs(),
        old_tabs,
        "a visual fixture never closes native objects"
    );
    assert_eq!(
        harness.state().state().localizer.diagnostics(),
        Default::default()
    );
    // A fitting front confirmation must not hide a broken underlying Options
    // dialog. Both retained owners must fit whenever both are presented.
    if harness.state().state().options_dialog_open() {
        area(harness, "application_options");
    }
    match mode {
        "close" => {
            let modal = area(harness, "database_options_close_objects");
            painted(harness, "database-options-close-title", modal);
            button(harness, "database-options-close-continue", false, modal);
            button(harness, "database-options-close-cancel", false, modal);
        }
        "review" => {
            let modal = area(harness, "database_options_review");
            painted(harness, "database-options-reviewing", modal);
            button(harness, "editing-request-cancellation", false, modal);
        }
        _ => {
            let modal = area(harness, "application_options");
            painted(harness, "settings-title", modal);
            painted(harness, "database-options-category-general", modal);
            if mode == "general" {
                painted(harness, "settings-appearance", modal);
            } else {
                button(harness, "database-options-disable", true, modal);
                button(harness, "database-options-enable-unavailable", true, modal);
                button(harness, "database-options-log-unavailable", true, modal);
                if matches!(mode, "unknown" | "refused") {
                    assert_eq!(harness.state().state().database_options_observation(), None);
                    painted(harness, "database-options-state-unknown", modal);
                }
                if mode == "refused" {
                    button(harness, "database-options-retry-inspection", false, modal);
                }
                if mode == "returned" {
                    painted(harness, "database-options-saved-warning", modal);
                    button(harness, "database-options-hide-details", false, modal);
                    button(harness, "backstage-close", false, modal);
                    assert!(matches!(
                        harness
                            .state()
                            .state()
                            .database_options_save_status()
                            .unwrap()
                            .1,
                        Some(EditSaveOutcome::Saved(_))
                    ));
                }
            }
        }
    }
}

fn settle(harness: &mut Harness<'_, Preview>) {
    harness.run_steps(LAYOUT_SETTLE_STEPS);
    if !harness.state().state().busy {
        harness.run();
    }
    harness.remove_cursor();
}

fn area(harness: &Harness<'_, Preview>, id: &str) -> egui::Rect {
    let modal = harness
        .ctx
        .memory(|memory| memory.area_rect(Id::new(id)))
        .expect("actual Options modal area");
    assert!(
        harness.ctx.viewport_rect().contains_rect(modal),
        "{id} rect={modal:?} must fit viewport={:?}",
        harness.ctx.viewport_rect()
    );
    modal
}

fn button(harness: &Harness<'_, Preview>, key: &str, disabled: bool, modal: egui::Rect) {
    let label = harness.state().state().localizer.text(key);
    let buttons = harness
        .query_all_by_role_and_label(Role::Button, &label)
        .filter(|button| modal.contains_rect(button.rect()))
        .collect::<Vec<_>>();
    assert_eq!(buttons.len(), 1, "one {key} belongs to the Options modal");
    assert_eq!(buttons[0].accesskit_node().is_disabled(), disabled, "{key}");
    painted(harness, key, modal);
}

fn painted(harness: &Harness<'_, Preview>, key: &str, owner: egui::Rect) {
    let label = harness.state().state().localizer.text(key);
    assert!(
        harness.output().shapes.iter().any(|shape| {
            let Shape::Text(text) = &shape.shape else {
                return false;
            };
            text.galley.job.text == label
                && owner.contains_rect(egui::Rect::from_min_size(text.pos, text.galley.size()))
        }),
        "complete {key} is painted inside its actual owner"
    );
}

//! Source-free Number failure/return illustrations; no native actor or I/O exists.
//! No Access capture of these surfaces is retained; these are UI adaptations.
use super::{assert_painted_in, settle};
use egui::accesskit::Role;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use tablecraft_builders::table::FieldAdditionKind;
use tablecraft_ui::{EditSaveOutcome, SaveCleanupNotice, preview::Preview};

const REVIEW_REQUEST_ID: u64 = 811;
const REVIEW_ID: u64 = 812;
// Presentation fact only. No SaveWorker, capability, database or journal exists.
const PRESENTATION_SAVE_ID: u64 = 813;

pub(super) fn is_mode(mode: &str) -> bool {
    matches!(
        mode,
        "refresh-failed" | "returned-warning" | "returned-close"
    )
}

pub(super) fn prepare(harness: &mut Harness<'_, Preview>, mode: &str) {
    let (tab, identity) = {
        let state = harness.state_mut().state_mut();
        let request = state
            .field_addition_review_request()
            .expect("real header name snapshot");
        let tab = request.tab.clone();
        assert!(state.begin_field_addition_review(&request, REVIEW_REQUEST_ID));
        assert!(state.accept_field_addition_review(&request, REVIEW_REQUEST_ID, REVIEW_ID));
        let identity = state
            .prepare_field_addition_save(&request, REVIEW_REQUEST_ID, REVIEW_ID)
            .expect("exclusive presentation preparation")
            .bind(PRESENTATION_SAVE_ID);
        let sample = state.localizer.text("editing-mock-diagnosis");
        assert!(
            state.set_field_addition_save_outcome(identity, EditSaveOutcome::Saved(sample.clone()))
        );
        assert!(state.begin_field_addition_reopen(identity));
        assert!(state.fail_field_addition_reopen(identity, sample));
        assert!(state.can_return_to_field_addition_draft(identity));
        let mut stale = identity;
        stale.save_id += 1;
        assert!(!state.return_field_addition_to_draft(&request, stale, None));
        if mode != "refresh-failed" {
            assert!(state.return_field_addition_to_draft(
                &request,
                identity,
                Some(SaveCleanupNotice::RetainedPrivateJournal)
            ));
            assert!(state.field_addition_source_stale());
            assert!(!state.can_review_field_addition());
            assert!(!state.can_edit_field_addition_name(&tab));
            assert!(state.field_addition_save_status().is_none());
            assert_eq!(
                state.field_addition_saved_warning_name().unwrap(),
                (&tab, "Field1")
            );
            let warning = state.field_addition_saved_warning().unwrap();
            assert_eq!(warning.identity, identity);
            assert!(matches!(warning.outcome, Some(EditSaveOutcome::Saved(_))));
            assert_eq!(
                warning.cleanup,
                Some(SaveCleanupNotice::RetainedPrivateJournal)
            );
            if mode == "returned-close" {
                assert!(state.request_field_addition_close(&tab));
            }
        }
        (tab, identity)
    };
    settle(harness);
    if mode == "refresh-failed" {
        assert!(
            harness
                .state()
                .state()
                .can_return_to_field_addition_draft(identity)
        );
        let modal = harness
            .ctx
            .memory(|memory| memory.area_rect(egui::Id::new("field_addition_save_status")))
            .expect("actual Number status modal");
        assert!(harness.ctx.viewport_rect().contains_rect(modal));
        for key in [
            "saved-query-save-retry-reopen",
            "datasheet-field-return-draft",
        ] {
            assert_button(harness, key, false, Some(modal));
        }
    } else if mode == "returned-close" {
        assert!(!harness.state().state().can_save_field_addition_close());
        let modal = harness
            .ctx
            .memory(|memory| memory.area_rect(egui::Id::new("field_addition_close")))
            .expect("actual saved-warning close modal");
        assert!(harness.ctx.viewport_rect().contains_rect(modal));
        for key in [
            "datasheet-field-abandon-title",
            "datasheet-field-abandon-question",
        ] {
            assert_painted_in(harness, &harness.state().state().localizer.text(key), modal);
        }
        assert_button(harness, "datasheet-field-close-save", true, Some(modal));
        assert_button(
            harness,
            "datasheet-field-abandon-discard",
            false,
            Some(modal),
        );
        assert_button(harness, "datasheet-field-close-cancel", false, Some(modal));
    } else {
        let state = harness.state().state();
        assert_painted_in(
            harness,
            &state.localizer.text("datasheet-field-saved-warning"),
            harness.ctx.viewport_rect(),
        );
        let label = state.localizer.text("table-draft-field-name");
        let editor = harness.get_by_role_and_label(Role::TextInput, &label);
        assert!(editor.accesskit_node().is_disabled());
        assert_eq!(editor.value().as_deref(), Some("Field1"));
        assert!(!state.can_name_field_addition(&tab, FieldAdditionKind::Number));
    }
}

fn assert_button(
    harness: &Harness<'_, Preview>,
    key: &str,
    disabled: bool,
    modal: Option<egui::Rect>,
) {
    let label = harness.state().state().localizer.text(key);
    let buttons = harness
        .query_all_by_role_and_label(Role::Button, &label)
        .filter(|button| modal.is_none_or(|modal| modal.contains_rect(button.rect())))
        .collect::<Vec<_>>();
    assert_eq!(buttons.len(), 1, "one bound {key} control");
    assert_eq!(buttons[0].accesskit_node().is_disabled(), disabled);
    assert_painted_in(
        harness,
        &label,
        modal.unwrap_or_else(|| harness.ctx.viewport_rect()),
    );
}

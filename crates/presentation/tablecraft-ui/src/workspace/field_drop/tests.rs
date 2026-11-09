//! Source-free presentation/ownership checks, never native eligibility witnesses.
use super::*;
use crate::workspace::{EditSaveOutcome, SaveCleanupNotice, test_support};
use crate::{Column, Database, State, Table};

fn confirming() -> (State, FieldDropTarget) {
    let mut state = test_support::saved_table(
        "DROP ownership",
        vec![
            Column {
                name: "ID".into(),
                caption: None,
                native_number: Some(0),
            },
            Column {
                name: "Score".into(),
                caption: Some("Friendly".into()),
                native_number: Some(17),
            },
        ],
    );
    state.field_drop_available = true;
    let target = FieldDropTarget {
        tab: state.active_object().unwrap(),
        column: "Score".into(),
        native_number: 17,
    };
    assert!(state.begin_field_drop(target.copy().unwrap()));
    (state, target)
}

fn observed() -> (State, FieldDropTarget) {
    let (mut state, target) = confirming();
    state
        .prepare_field_drop_inspection(&target)
        .unwrap()
        .bind(80);
    assert!(state.set_field_drop_inspection_by_id(80, 7, "Table1", "Score", 17, true));
    (state, target)
}

fn pending() -> (State, FieldDropRequest) {
    let (mut state, target) = observed();
    assert!(
        state
            .choose_field_drop_confirmation(&target, 0, FieldDropConfirmationDecision::Yes)
            .is_some()
    );
    let request = state.field_drop_review_request().unwrap();
    assert_eq!(request.draft_revision, 1);
    (state, request)
}

fn accepted() -> (State, FieldDropRequest) {
    let (mut state, request) = pending();
    assert!(state.begin_field_drop_review(&request, 81));
    assert!(state.accept_field_drop_review(&request, 81, 90));
    (state, request)
}

fn saving() -> (State, FieldDropRequest, FieldDropSaveIdentity) {
    let (mut state, request) = accepted();
    let identity = state
        .prepare_field_drop_save(&request, 81, 90)
        .unwrap()
        .bind(100);
    (state, request, identity)
}

fn fresh() -> Database {
    Database {
        document_revision: 8,
        title_path: None,
        title: "DROP ownership".into(),
        format: "ACE 16".into(),
        queries: Vec::new(),
        tables: vec![Table {
            name: "Table1".into(),
            local: true,
            system: false,
            storage: "Local".into(),
        }],
    }
}

#[test]
fn exact_metadata_not_caption_or_position_controls_begin_and_confirmation() {
    let (mut state, target) = confirming();
    assert!(!state.field_drop_dirty());
    assert!(state.pending_field_drop().is_none());
    assert!(state.field_drop_review_request().is_none());
    assert!(!state.can_confirm_field_drop(&target, 0));
    state
        .prepare_field_drop_inspection(&target)
        .unwrap()
        .bind(80);
    let mut wrong = target.copy().unwrap();
    wrong.native_number = 1;
    assert!(!state.set_field_drop_source(&wrong, 80, true));
    wrong = target.copy().unwrap();
    wrong.tab.id += 1;
    assert!(!state.set_field_drop_source(&wrong, 80, true));
    assert!(!state.set_field_drop_inspection_by_id(80, 8, "Table1", "Score", 17, true));
    assert!(!state.set_field_drop_inspection_by_id(80, 7, "Table1", "Friendly", 17, true));
    assert!(!state.set_field_drop_source(&target, 81, true));
    assert!(state.set_field_drop_source(&target, 80, true));
    assert!(!state.can_confirm_field_drop(&wrong, 0));
    assert!(
        state
            .choose_field_drop_confirmation(&target, 0, FieldDropConfirmationDecision::Yes)
            .is_some()
    );
    assert!(
        state
            .choose_field_drop_confirmation(&target, 0, FieldDropConfirmationDecision::Yes)
            .is_none()
    );
    assert!(state.field_drop_dirty());
    assert_eq!(state.pending_field_drop().unwrap().1, 1);
    assert_eq!(state.data.as_ref().unwrap().columns.len(), 2);
    assert_eq!(state.data.as_ref().unwrap().columns[1].name, "Score");
    assert_eq!(state.database.as_ref().unwrap().document_revision, 7);
}

#[test]
fn no_and_unstarted_inspection_cancel_only_the_exact_clean_presentation() {
    let (mut state, target) = confirming();
    drop(state.prepare_field_drop_inspection(&target).unwrap());
    assert!(state.retry_field_drop_inspection(&target));
    state
        .prepare_field_drop_inspection(&target)
        .unwrap()
        .bind(80);
    let result = state
        .choose_field_drop_confirmation(&target, 0, FieldDropConfirmationDecision::No)
        .unwrap();
    assert_eq!(result.inspection_request_id, Some(80));
    assert!(!state.field_drop_open());
    assert!(!state.field_drop_dirty());
    assert!(!state.set_field_drop_source(&target, 80, true));
    assert_eq!(state.data.as_ref().unwrap().columns.len(), 2);
    let mut caption = target.copy().unwrap();
    caption.column = "Friendly".into();
    assert!(!state.begin_field_drop(caption));
    let mut revision = target.copy().unwrap();
    revision.tab.target.document_revision = 8;
    assert!(!state.begin_field_drop(revision));
}

#[test]
fn dirty_close_save_discard_cancel_and_stale_revision_are_exact() {
    let (mut state, request) = pending();
    assert!(state.request_current_field_drop_close());
    let cancel = state
        .choose_field_drop_close(FieldDropCloseDecision::Cancel)
        .unwrap();
    assert!(cancel.request.is_none());
    assert!(state.field_drop_dirty());
    assert!(state.request_current_field_drop_close());
    let save = state
        .choose_field_drop_close(FieldDropCloseDecision::Save)
        .unwrap();
    assert_eq!(save.request.unwrap(), request);
    assert!(state.field_drop_dirty());
    assert!(state.field_drop_save_status().is_none());
    assert!(state.request_current_field_drop_close());
    state.field_drop.draft.as_mut().unwrap().revision += 1;
    assert!(
        state
            .choose_field_drop_close(FieldDropCloseDecision::Discard)
            .is_none()
    );
    assert!(
        state
            .choose_field_drop_close(FieldDropCloseDecision::Cancel)
            .is_some()
    );
    assert!(state.request_current_field_drop_close());
    assert!(
        state
            .choose_field_drop_close(FieldDropCloseDecision::Discard)
            .is_some()
    );
    assert!(!state.field_drop_open());
    assert_eq!(state.data.as_ref().unwrap().columns[1].name, "Score");
}

#[test]
fn refused_save_start_keeps_confirmed_delete_and_binding_is_allocation_free() {
    let (mut state, request) = accepted();
    assert!(!state.accept_field_drop_review(&request, 82, 90));
    drop(state.prepare_field_drop_save(&request, 81, 90).unwrap());
    assert!(state.can_review_field_drop());
    assert!(state.field_drop_dirty());
    assert!(state.field_drop_save_status().is_none());
    assert!(state.begin_field_drop_review(&request, 82));
    assert!(state.accept_field_drop_review(&request, 82, 91));
    let bytes = state.object_bytes();
    let identity = state
        .prepare_field_drop_save(&request, 82, 91)
        .unwrap()
        .bind(101);
    assert_eq!(state.object_bytes(), bytes);
    assert!(state.matches_field_drop_save(&request, identity));
    assert!(!state.can_review_field_drop());
    assert!(!state.can_confirm_field_drop(&request.target, request.draft_revision));
}

#[test]
fn retained_outcome_requires_explicit_resolution_and_original_release_stays_dirty() {
    let (mut state, request, identity) = saving();
    assert!(state.set_field_drop_save_outcome(
        identity,
        EditSaveOutcome::Retained {
            message: "held real resource".into(),
            can_recover: true,
            can_release: false,
        }
    ));
    assert!(!state.can_acknowledge_field_drop_save(identity));
    assert!(!state.begin_field_drop_reopen(identity));
    assert!(state.can_resolve_field_drop_save(identity, true));
    assert!(!state.can_resolve_field_drop_save(identity, false));
    drop(
        state
            .prepare_field_drop_resolution(&request, identity, true)
            .unwrap(),
    );
    assert!(state.can_resolve_field_drop_save(identity, true));
    let next = state
        .prepare_field_drop_resolution(&request, identity, true)
        .unwrap()
        .bind(102);
    assert!(!state.set_field_drop_save_outcome(identity, EditSaveOutcome::Saved("stale".into())));
    assert!(
        state.set_field_drop_save_outcome(
            next,
            EditSaveOutcome::Cancelled("original verified".into())
        )
    );
    // Source-free caller simulates the host's original/no-write ACK prerequisite.
    assert!(state.release_field_drop_save(&request, next));
    assert!(state.field_drop_dirty());
    assert!(state.can_review_field_drop());
    assert!(state.field_drop_save_status().is_none());
    assert_eq!(state.data.as_ref().unwrap().columns.len(), 2);
}

#[test]
fn failed_known_after_read_retains_warning_cleanup_and_revision_scoped_stale_guard() {
    let (mut state, request, identity) = saving();
    assert!(
        state.set_field_drop_save_outcome(identity, EditSaveOutcome::Saved("known AFTER".into()))
    );
    assert!(state.begin_field_drop_reopen(identity));
    assert!(state.cancel_field_drop_operation(identity));
    assert!(!state.cancel_field_drop_operation(identity));
    assert!(state.fail_field_drop_reopen(identity, "full witness read failed".into()));
    assert!(state.return_field_drop_to_draft(
        &request,
        identity,
        Some(SaveCleanupNotice::RetainedPrivateJournal)
    ));
    assert!(state.field_drop_source_stale());
    assert!(state.native_source_stale());
    assert!(!state.can_review_field_drop());
    let warning = state.field_drop_warning().unwrap();
    assert!(matches!(warning.outcome, Some(EditSaveOutcome::Saved(_))));
    assert_eq!(
        warning.cleanup,
        Some(SaveCleanupNotice::RetainedPrivateJournal)
    );
    assert!(state.request_current_field_drop_close());
    assert!(!state.can_save_field_drop_close());
    assert!(
        state
            .choose_field_drop_close(FieldDropCloseDecision::Discard)
            .is_some()
    );
    assert!(!state.field_drop_open());
    assert!(state.field_drop_source_stale());
    assert!(state.field_drop_warning().is_some());
    state.retire_field_drop_warning(Some(7));
    assert!(state.field_drop_warning().is_some());
    state.retire_field_drop_warning(Some(8));
    assert!(state.field_drop.warning.is_none());
}

#[test]
fn publication_requires_reopening_exact_identity_and_matching_catalog_then_invalidates_caches() {
    let (mut state, request, identity) = saving();
    let tabs = state.object_tabs();
    assert!(!state.resolve_field_drop_after_reopen(&request, identity, fresh()));
    assert!(
        state.set_field_drop_save_outcome(identity, EditSaveOutcome::Saved("known AFTER".into()))
    );
    assert!(!state.resolve_field_drop_after_reopen(&request, identity, fresh()));
    assert!(state.begin_field_drop_reopen(identity));
    let mut wrong = fresh();
    wrong.tables[0].name = "Foreign".into();
    let bytes = state.object_bytes();
    assert!(!state.resolve_field_drop_after_reopen(&request, identity, wrong));
    assert_eq!(state.object_tabs(), tabs);
    assert_eq!(state.object_bytes(), bytes);
    assert!(state.field_drop_dirty());
    // This source-free handoff exercises UI ownership only. It is no native
    // witness; production caller must satisfy the documented runtime prerequisites.
    assert!(state.resolve_field_drop_after_reopen(&request, identity, fresh()));
    assert_eq!(state.active_object().unwrap().id, request.target.tab.id);
    assert_eq!(state.database.as_ref().unwrap().document_revision, 8);
    assert!(state.data.is_none());
    assert!(state.schema.is_none());
    assert!(!state.field_drop_open());
    assert!(!state.field_drop_dirty());
    assert!(state.field_drop_save_status().is_none());
}

#[test]
fn all_retained_string_capacities_are_counted_and_oom_keeps_terminal_controls() {
    let (mut state, target) = confirming();
    state
        .choose_field_drop_confirmation(&target, 0, FieldDropConfirmationDecision::No)
        .unwrap();
    let mut oversized = target.copy().unwrap();
    oversized
        .column
        .reserve_exact(crate::workspace::objects::MAX_RETAINED_BYTES);
    assert!(!state.begin_field_drop(oversized));
    assert!(!state.field_drop_open());
    let (mut state, request, identity) = saving();
    let mut message = String::with_capacity(crate::workspace::objects::MAX_RETAINED_BYTES + 1);
    message.push_str("retained");
    assert!(state.set_field_drop_save_outcome(
        identity,
        EditSaveOutcome::Retained {
            message,
            can_recover: true,
            can_release: false,
        }
    ));
    assert!(state.can_resolve_field_drop_save(identity, true));
    assert!(!state.can_resolve_field_drop_save(identity, false));
    assert!(!state.can_acknowledge_field_drop_save(identity));
    assert!(state.matches_field_drop_save(&request, identity));
}

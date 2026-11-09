//! Source-free UI ownership checks, not native qualification or fake save authority.
use super::FieldRenameSaveIdentity;
use crate::workspace::EditSaveOutcome;

use super::test_support::{accepted, naming};

#[test]
fn actual_number_true_name_and_source_facts_are_independent_of_captions_and_ordinals() {
    let (mut state, target) = naming();
    assert!(!state.can_edit_field_rename());
    assert!(!state.native_source_stale());
    assert!(state.native_mutation_blocked());
    assert!(!state.object_switch_ready());
    let outgoing = state.field_rename_inspection_target().unwrap();
    assert_eq!(outgoing.native_number, 17);
    assert_eq!(outgoing.column, "Score");
    state
        .prepare_field_rename_inspection(&outgoing)
        .unwrap()
        .bind(80);
    assert!(!state.set_field_rename_inspection_by_id(
        80,
        7,
        "Table1",
        "Friendly",
        17,
        true,
        Some(5)
    ));
    assert!(!state.set_field_rename_inspection_by_id(80, 7, "Table1", "Score", 1, true, Some(5)));
    state.busy = true;
    assert!(state.set_field_rename_inspection_by_id(80, 7, "Table1", "Score", 17, true, Some(5)));
    state.busy = false;
    assert!(state.can_edit_field_rename());
    assert!(state.set_field_rename_name(&target, "Longer".into()));
    assert!(!state.can_review_field_rename());
    assert_eq!(state.field_rename_proposal().unwrap().1, "Longer");
}

#[test]
fn abandoned_read_preparation_and_revoked_publication_allow_retry_without_losing_names() {
    let (mut state, target) = naming();
    drop(state.prepare_field_rename_inspection(&target).unwrap());
    assert!(state.retry_field_rename_inspection(&target));
    state
        .prepare_field_rename_inspection(&target)
        .unwrap()
        .bind(80);
    assert_eq!(state.pending_field_rename_inspection_id(), Some(80));
    assert!(!state.interrupt_field_rename_inspection_id(79));
    assert!(state.interrupt_field_rename_inspection_id(80));
    assert_eq!(state.field_rename_proposal().unwrap().1, "Score");
    assert!(state.retry_field_rename_inspection(&target));
}

#[test]
fn actor_and_unchanged_preparations_are_distinct_and_binding_adds_no_heap() {
    let (mut changed, request) = accepted(true);
    assert!(
        changed
            .prepare_unchanged_field_rename(&request, 81, 90)
            .is_none()
    );
    let before = changed.object_bytes().unwrap();
    let identity = changed
        .prepare_field_rename_save(&request, 81, 90)
        .unwrap()
        .bind(100);
    assert!(matches!(
        identity,
        FieldRenameSaveIdentity::NativeSave { save_id: 100, .. }
    ));
    assert!(changed.matches_field_rename_save(&request, identity));
    assert_eq!(changed.object_bytes(), Some(before));

    let (mut unchanged, request) = accepted(false);
    assert!(
        unchanged
            .prepare_field_rename_save(&request, 81, 90)
            .is_none()
    );
    let identity = unchanged
        .prepare_unchanged_field_rename(&request, 81, 90)
        .unwrap()
        .bind();
    assert!(matches!(
        identity,
        FieldRenameSaveIdentity::UnchangedReview { .. }
    ));
    assert!(unchanged.matches_field_rename_save(&request, identity));
    assert!(
        !unchanged.set_field_rename_save_outcome(identity, EditSaveOutcome::Saved("fake".into()))
    );
    assert!(!unchanged.can_acknowledge_field_rename_save(identity));
    assert!(!unchanged.can_resolve_field_rename_save(identity, false));
}

#[test]
fn refused_start_and_resolution_leave_the_same_name_and_terminal_owner() {
    let (mut state, request) = accepted(true);
    drop(state.prepare_field_rename_save(&request, 81, 90).unwrap());
    assert_eq!(state.field_rename_review_request().unwrap(), request);
    assert!(state.begin_field_rename_review(&request, 82));
    assert!(state.accept_field_rename_review(&request, 82, 91));
    let identity = state
        .prepare_field_rename_save(&request, 82, 91)
        .unwrap()
        .bind(100);
    assert!(state.set_field_rename_save_outcome(
        identity,
        EditSaveOutcome::Retained {
            message: "retain exact outcome".into(),
            can_recover: true,
            can_release: true,
        }
    ));
    {
        let _preparation = state
            .prepare_field_rename_resolution(&request, identity, true)
            .unwrap();
    }
    assert!(state.can_resolve_field_rename_save(identity, true));
    let next = state
        .prepare_field_rename_resolution(&request, identity, false)
        .unwrap()
        .bind(101);
    assert!(!state.matches_field_rename_save(&request, identity));
    assert!(state.matches_field_rename_save(&request, next));
    assert_eq!(state.field_rename_proposal().unwrap().1, "Total");
}

#[test]
fn failed_witness_return_moves_exact_name_buffers_and_keeps_stale_guard_after_discard() {
    let (mut state, request) = accepted(true);
    let identity = state
        .prepare_field_rename_save(&request, 81, 90)
        .unwrap()
        .bind(100);
    assert!(
        state.set_field_rename_save_outcome(identity, EditSaveOutcome::Saved("known AFTER".into()))
    );
    assert!(state.begin_field_rename_reopen(identity));
    assert!(state.fail_field_rename_reopen(identity, "full-image read failed".into()));
    let before = state.field_rename_proposal().unwrap().1.as_ptr();
    assert!(state.return_field_rename_to_draft(&request, identity, None));
    assert_eq!(state.field_rename_proposal().unwrap().1.as_ptr(), before);
    assert!(state.native_source_stale());
    assert!(!state.can_edit_field_rename());
    assert!(state.discard_field_rename(&request.target, request.draft_revision));
    assert!(state.field_rename_proposal().is_none());
    assert!(state.native_source_stale());
    let warning = state.field_rename_warning().unwrap();
    assert_eq!(warning.proposed_name, "Total");
    assert_eq!(warning.proposed_name.as_ptr(), before);
    assert_eq!(warning.reopen_error, Some("full-image read failed"));
}

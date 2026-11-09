//! Local state choices do not represent native save or witness authority.
use super::FieldRenameCloseDecision as Decision;
use crate::workspace::field_rename::test_support::{accepted, naming, observed};

#[test]
fn clean_editor_closes_without_prompt_and_retires_only_its_read() {
    let (mut state, target) = naming();
    state
        .prepare_field_rename_inspection(&target)
        .unwrap()
        .bind(80);
    let prepared = state.prepare_clean_field_rename_close().unwrap();
    assert_eq!(prepared.inspection_request_id(), Some(80));
    prepared.complete();
    assert!(!state.field_rename_open());
    assert!(!state.field_rename_close_open());
}

#[test]
fn dirty_close_cancel_and_save_preserve_exact_proposal_and_revision() {
    let (mut state, target) = observed();
    assert!(state.set_field_rename_name(&target, "Total".into()));
    let revision = state.field_rename_proposal().unwrap().2;
    assert!(state.prepare_clean_field_rename_close().is_none());
    assert!(state.request_current_field_rename_close());
    let result = state.choose_field_rename_close(Decision::Cancel).unwrap();
    assert_eq!(result.draft_revision, revision);
    assert!(result.request.is_none());
    assert_eq!(state.field_rename_proposal().unwrap().1, "Total");

    assert!(state.request_current_field_rename_close());
    let result = state.choose_field_rename_close(Decision::Save).unwrap();
    let request = result.request.unwrap();
    assert_eq!(request.target, target);
    assert_eq!(request.draft_revision, revision);
    assert_eq!(request.new_name, "Total");
    assert!(state.field_rename_request_is_current(&request));
    assert_eq!(state.field_rename_proposal().unwrap().1, "Total");
}

#[test]
fn stale_close_never_discards_newer_text_and_unsupported_text_can_be_discarded() {
    let (mut state, target) = observed();
    assert!(state.set_field_rename_name(&target, "Longer".into()));
    assert!(state.request_current_field_rename_close());
    assert!(!state.can_save_field_rename_close());
    // Inject a new revision across the confirmation boundary; ordinary widget
    // admissions already block this mutation while the modal is open.
    state.field_rename.draft.as_mut().unwrap().revision += 1;
    assert!(state.choose_field_rename_close(Decision::Discard).is_none());
    assert_eq!(state.field_rename_proposal().unwrap().1, "Longer");
    assert!(state.choose_field_rename_close(Decision::Cancel).is_some());
    assert!(state.request_current_field_rename_close());
    assert!(state.choose_field_rename_close(Decision::Discard).is_some());
    assert!(!state.field_rename_open());
}

#[test]
fn failed_after_read_close_retains_warning_and_stale_source_guard() {
    let (mut state, request) = accepted(true);
    let identity = state
        .prepare_field_rename_save(&request, 81, 90)
        .unwrap()
        .bind(100);
    assert!(state.set_field_rename_save_outcome(
        identity,
        crate::EditSaveOutcome::Saved("known AFTER".into())
    ));
    assert!(state.begin_field_rename_reopen(identity));
    assert!(state.fail_field_rename_reopen(identity, "full-image mismatch".into()));
    assert!(state.return_field_rename_to_draft(&request, identity, None));
    assert!(state.request_current_field_rename_close());
    assert!(!state.can_save_field_rename_close());
    assert!(state.choose_field_rename_close(Decision::Discard).is_some());
    assert!(!state.field_rename_open());
    assert!(state.field_rename_source_stale());
    let warning = state.field_rename_warning().unwrap();
    assert_eq!(warning.proposed_name, "Total");
    assert_eq!(warning.reopen_error, Some("full-image mismatch"));
}

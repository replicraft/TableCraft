//! Source-free presentation safety; runtime/source qualification is tested by hosts.
use super::{confirming, saving};
use crate::{EditSaveOutcome, FieldDropSaveIdentity};

#[test]
fn late_or_unsupported_inspections_never_confirm_or_create_a_save_request() {
    let (mut state, target) = confirming();
    state
        .prepare_field_drop_inspection(&target)
        .unwrap()
        .bind(80);
    assert!(!state.set_field_drop_inspection_by_id(79, 7, "Table1", "Score", 17, true));
    assert!(!state.set_field_drop_inspection_by_id(80, 7, "Table1", "Score", 1, true));
    assert!(!state.can_confirm_field_drop(&target, 0));
    assert!(state.field_drop_review_request().is_none());

    assert!(state.set_field_drop_inspection_by_id(80, 7, "Table1", "Score", 17, false));
    assert!(!state.can_confirm_field_drop(&target, 0));
    assert!(state.field_drop_review_request().is_none());
    assert!(!state.field_drop_dirty());
    assert_eq!(state.data.as_ref().unwrap().columns[1].name, "Score");
    assert_eq!(
        state.data.as_ref().unwrap().columns[1].native_number,
        Some(17)
    );
}

#[test]
fn actor_loss_replaces_retained_rights_and_ack_keeps_pending_deletion_and_stale_source() {
    let (mut state, request, identity) = saving();
    assert!(state.set_field_drop_save_outcome(
        identity,
        EditSaveOutcome::Retained {
            message: "illustrative retained owner".into(),
            can_recover: true,
            can_release: true,
        },
    ));
    assert!(state.can_resolve_field_drop_save(identity, true));
    assert!(state.can_resolve_field_drop_save(identity, false));

    let stale = FieldDropSaveIdentity {
        save_id: identity.save_id + 1,
        ..identity
    };
    assert!(!state.set_field_drop_actor_lost(stale, "other actor".into()));
    assert!(state.can_resolve_field_drop_save(identity, true));
    assert!(state.set_field_drop_actor_lost(identity, "outcome is uncertain".into()));
    assert!(!state.can_resolve_field_drop_save(identity, true));
    assert!(!state.can_resolve_field_drop_save(identity, false));
    assert!(state.can_acknowledge_field_drop_save(identity));
    assert!(state.native_source_stale());
    assert!(state.field_drop.request_memory.is_some());
    assert!(!state.acknowledge_uncertain_field_drop_save(&request, stale));

    assert!(state.acknowledge_uncertain_field_drop_save(&request, identity));
    assert!(state.field_drop.request_memory.is_none());
    assert!(state.field_drop_save_status().is_none());
    assert!(state.field_drop_dirty());
    assert_eq!(state.pending_field_drop().unwrap().0, &request.target);
    assert_eq!(state.data.as_ref().unwrap().columns[1].name, "Score");
    assert_eq!(state.database.as_ref().unwrap().document_revision, 7);
    assert!(!state.can_review_field_drop());
    assert!(!state.can_retry_field_drop_reopen(identity));
    assert!(state.native_mutation_blocked());

    assert!(state.request_current_field_drop_close());
    assert!(
        state
            .choose_field_drop_close(crate::FieldDropCloseDecision::Discard)
            .is_some()
    );
    assert!(!state.field_drop_open());
    assert!(state.native_source_stale());
    assert!(state.native_mutation_blocked());
    state.database.as_mut().unwrap().document_revision = 8;
    assert!(!state.native_source_stale());
}

#[test]
fn actor_loss_at_payload_ceiling_remains_ackable_without_extra_target_copies() {
    let (mut state, request, identity) = saving();
    let target_pointer = state.field_drop_target().unwrap().column.as_ptr();
    state.sql = String::with_capacity(crate::workspace::objects::MAX_RETAINED_BYTES);
    assert!(state.set_field_drop_actor_lost(identity, "outcome is uncertain".into()));
    assert!(state.can_acknowledge_field_drop_save(identity));
    assert!(!state.can_resolve_field_drop_save(identity, true));
    assert!(!state.can_resolve_field_drop_save(identity, false));
    assert!(matches!(
        state.field_drop_save_status().unwrap().1,
        Some(EditSaveOutcome::Failed(message)) if message.is_empty()
    ));
    assert!(state.field_drop.status.as_ref().unwrap().omitted);
    assert!(state.acknowledge_uncertain_field_drop_save(&request, identity));
    assert_eq!(
        state.field_drop_target().unwrap().column.as_ptr(),
        target_pointer
    );
    assert!(state.field_drop.request_memory.is_none());
    assert!(state.field_drop_dirty());
    assert!(state.native_mutation_blocked());
}

#[test]
fn disabling_host_support_after_entry_suppresses_inspection_but_keeps_no_available() {
    let (mut state, target) = confirming();
    assert!(state.field_drop_inspection_target().is_some());
    state.field_drop_available = false;
    assert!(state.field_drop_inspection_target().is_none());
    assert!(state.prepare_field_drop_inspection(&target).is_none());
    assert!(!state.can_confirm_field_drop(&target, 0));
    assert!(state.field_drop_review_request().is_none());
    assert!(
        state
            .choose_field_drop_confirmation(&target, 0, crate::FieldDropConfirmationDecision::No,)
            .is_some()
    );
    assert!(!state.field_drop_open());
    assert_eq!(state.data.as_ref().unwrap().columns[1].name, "Score");
}

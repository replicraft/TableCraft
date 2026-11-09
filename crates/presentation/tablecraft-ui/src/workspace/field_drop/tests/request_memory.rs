use super::{accepted, fresh, pending, saving};
use crate::workspace::{EditSaveOutcome, objects::MAX_RETAINED_BYTES};

#[test]
fn exact_request_memory_remains_reserved_through_resolution_and_failed_reopen() {
    let (mut state, request) = pending();
    let baseline = state.object_bytes().unwrap();
    let request_bytes = request.retained_bytes().unwrap();
    assert!(state.begin_field_drop_review(&request, 81));
    assert_eq!(state.object_bytes(), Some(baseline + request_bytes));
    assert!(state.accept_field_drop_review(&request, 81, 90));
    let identity = state
        .prepare_field_drop_save(&request, 81, 90)
        .unwrap()
        .bind(100);
    assert_eq!(state.object_bytes(), Some(baseline + request_bytes));
    assert!(state.set_field_drop_save_outcome(
        identity,
        EditSaveOutcome::Retained {
            message: String::new(),
            can_recover: true,
            can_release: true,
        }
    ));
    assert_eq!(state.object_bytes(), Some(baseline + request_bytes));
    {
        let _unbound = state
            .prepare_field_drop_resolution(&request, identity, true)
            .unwrap();
    }
    assert_eq!(state.object_bytes(), Some(baseline + request_bytes));
    let next = state
        .prepare_field_drop_resolution(&request, identity, true)
        .unwrap()
        .bind(101);
    assert_eq!(state.object_bytes(), Some(baseline + request_bytes));
    assert!(state.set_field_drop_save_outcome(next, EditSaveOutcome::Saved(String::new())));
    assert!(state.begin_field_drop_reopen(next));
    assert!(state.fail_field_drop_reopen(next, String::new()));
    assert_eq!(state.object_bytes(), Some(baseline + request_bytes));
    assert!(state.return_field_drop_to_draft(&request, next, None));
    assert_eq!(state.object_bytes(), Some(baseline));
    assert!(state.field_drop_source_stale());
    assert!(state.field_drop_warning().is_some());
}

#[test]
fn review_rechecks_the_actual_snapshot_capacity_and_releases_only_the_exact_owner() {
    let (mut state, mut request) = pending();
    let baseline = state.object_bytes().unwrap();
    let previous_diagnostic = state.error.as_ref().map_or(0, String::capacity);
    request.target.column.reserve_exact(MAX_RETAINED_BYTES);
    assert!(!state.begin_field_drop_review(&request, 81));
    let diagnostic = state.error.as_ref().expect("quota refusal is visible");
    assert_eq!(diagnostic, &state.localizer.text("objects-memory-limit"));
    assert_eq!(
        state.object_bytes(),
        Some(baseline - previous_diagnostic + diagnostic.capacity()),
        "only the retained refusal diagnostic changes the object count"
    );
    assert!(state.field_drop.request_memory.is_none());
    assert!(state.can_review_field_drop());

    let (mut state, request) = pending();
    let baseline = state.object_bytes().unwrap();
    assert!(state.begin_field_drop_review(&request, 81));
    assert!(!state.release_field_drop_review(&request, 82));
    assert!(state.release_field_drop_review(&request, 81));
    assert_eq!(state.object_bytes(), Some(baseline));
    assert!(state.begin_field_drop_review(&request, 82));
    assert!(state.accept_field_drop_review(&request, 82, 90));
    assert!(!state.release_field_drop_accepted_review(&request, 81, 90));
    assert!(state.release_field_drop_accepted_review(&request, 82, 90));
    assert_eq!(state.object_bytes(), Some(baseline));
}

#[test]
fn abandoned_start_release_does_not_allocate_or_require_remaining_memory() {
    let (mut state, request) = accepted();
    let reserved = request.retained_bytes().unwrap();
    drop(state.prepare_field_drop_save(&request, 81, 90).unwrap());
    assert!(!state.can_review_field_drop());
    assert!(!state.request_current_field_drop_close());
    state.sql.reserve_exact(MAX_RETAINED_BYTES);
    let retained = state.object_bytes().unwrap();
    assert!(retained > MAX_RETAINED_BYTES);
    let pointers = (request.target.column.as_ptr(), state.sql.as_ptr());
    assert!(state.release_field_drop_review(&request, 81));
    assert_eq!(state.object_bytes(), Some(retained - reserved));
    assert_eq!(
        pointers,
        (request.target.column.as_ptr(), state.sql.as_ptr())
    );
    assert!(state.field_drop_dirty());
    assert!(state.field_drop_save_status().is_none());
}

#[test]
fn original_release_and_checked_publication_release_the_reservation() {
    let (mut state, request, identity) = saving();
    let before = state.object_bytes().unwrap();
    assert!(state.set_field_drop_save_outcome(identity, EditSaveOutcome::Cancelled(String::new())));
    assert!(state.release_field_drop_save(&request, identity));
    assert_eq!(
        state.object_bytes(),
        Some(before - request.retained_bytes().unwrap())
    );
    assert!(state.field_drop_dirty());

    let (mut state, request, identity) = saving();
    assert!(state.set_field_drop_save_outcome(identity, EditSaveOutcome::Saved(String::new())));
    assert!(state.begin_field_drop_reopen(identity));
    assert!(state.resolve_field_drop_after_reopen(&request, identity, fresh()));
    assert!(state.field_drop.request_memory.is_none());
    assert!(!state.field_drop_dirty());
}

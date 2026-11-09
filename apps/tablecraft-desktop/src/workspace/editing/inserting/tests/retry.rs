//! Real failed Apply → release/ack → guarded original reopen → new review.
use super::{begin, finish_reads, finish_save, opened, review_insert};
use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use tablecraft_runtime::SaveOutcome;
use tablecraft_testkit::support::access::retained_target::assert_retained_target_observation;

#[test]
fn no_write_failure_keeps_input_and_retry_refuses_drift_until_original_bytes_match() {
    let (_directory, path, original, mut harness) = opened();
    begin(&mut harness, "4");
    review_insert(&mut harness);
    let original_revision = harness
        .state()
        .state
        .database
        .as_ref()
        .unwrap()
        .document_revision;
    let mut changed = original.clone();
    *changed.last_mut().unwrap() ^= 1;
    std::fs::write(&path, &changed).unwrap();

    harness
        .get_by_role_and_label(Role::Button, "Apply new row to original file")
        .click_accesskit();
    harness.run_steps(1);
    let failure = finish_save(&mut harness);
    assert!(matches!(failure.outcome, SaveOutcome::Failed(_)));
    assert!(failure.stopped_before_target_writes());
    assert_retained_target_observation(&path, &changed);
    assert_eq!(
        harness
            .get_by_role_and_label(Role::MultilineTextInput, "Id")
            .value()
            .as_deref(),
        Some("4")
    );

    // A retained resource owner cannot be acknowledged away by a UI request.
    harness.state_mut().acknowledge_save(failure.id);
    assert!(harness.state().editing.has_save());
    assert_eq!(harness.state().editing.worker.poll(), Some(failure));
    harness
        .get_by_role_and_label(Role::Button, "Release save resources")
        .click_accesskit();
    harness.run_steps(1);
    let released = finish_save(&mut harness);
    assert!(matches!(released.outcome, SaveOutcome::Released { .. }));
    harness
        .get_by_role_and_label(Role::Button, "Close")
        .click_accesskit();
    harness.run_steps(2);
    assert!(!harness.state().editing.has_save());
    assert!(harness.state().state.insert_continuation_pending());
    assert!(harness.state().state.can_retry_insert(released.id));
    assert!(harness.state().state.database.is_none());

    // No automatic retry, source replacement or fresh review has occurred.
    assert!(harness.state().editing.insert_review.is_none());
    assert_eq!(std::fs::read(&path).unwrap(), changed);
    harness
        .get_by_role_and_label(Role::Button, "Reopen original and edit")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    assert!(harness.state().state.insert_continuation_pending());
    assert!(harness.state().state.database.is_none());
    assert_eq!(
        harness
            .get_by_role_and_label(Role::MultilineTextInput, "Id")
            .value()
            .as_deref(),
        Some("4")
    );
    assert_eq!(std::fs::read(&path).unwrap(), changed);

    // Restore only test-owned bytes after the actual native lease is released.
    std::fs::write(&path, &original).unwrap();
    harness
        .get_by_role_and_label(Role::Button, "Reopen original and edit")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    assert!(harness.state().state.inline_insert_active());
    assert!(!harness.state().state.insert_continuation_pending());
    assert_ne!(
        harness
            .state()
            .state
            .database
            .as_ref()
            .unwrap()
            .document_revision,
        original_revision
    );
    assert_eq!(
        harness
            .get_by_role_and_label(Role::TextInput, "Id")
            .value()
            .as_deref(),
        Some("4")
    );
    assert!(harness.state().editing.insert_review.is_none());
    assert_eq!(std::fs::read(&path).unwrap(), original);

    review_insert(&mut harness);
    harness
        .get_by_role_and_label(Role::Button, "Apply new row to original file")
        .click_accesskit();
    harness.run_steps(1);
    let saved = finish_save(&mut harness);
    assert!(
        matches!(saved.outcome, SaveOutcome::Saved { receipt } if receipt.changed_rows == Some(1))
    );
    harness
        .get_by_role_and_label(Role::Button, "Close")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    let data = harness.state().state.data.as_ref().unwrap();
    assert_eq!(data.rows.len(), 2);
    assert_eq!(data.rows[1][0].text, "4");
}

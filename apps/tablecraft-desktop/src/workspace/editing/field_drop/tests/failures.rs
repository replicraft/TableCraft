//! Real readonly refusal, cancelled review and retained saved-but-unverified state.
use super::{Entry, accept_review, choose, inspect, pending, take_review};
use crate::workspace::test_support::{self, finish_reads, finish_schema_save};
use egui::{Key, Modifiers, accesskit::Role};
use egui_kittest::kittest::{NodeT, Queryable};
use std::time::{Duration, Instant};
use tablecraft_runtime::{SaveOutcome, journal_path};
use tablecraft_testkit::fixtures::access::canonical_created_number::mixed::{
    self, after_off::deleted_field as raw,
};
use tablecraft_ui::{EditSaveOutcome, FieldDropSaveCommand};

#[test]
fn real_source_without_explicit_off_refuses_yes_without_changing_database_settings() {
    let before = mixed::source(mixed::SourceCase::TwoNumberHistory);
    let (_directory, path, _, mut harness) = test_support::opened(before.clone());
    inspect(&mut harness, "Score", Entry::Datasheet);
    let yes = harness.get_by_role_and_label(Role::Button, "Yes");
    assert!(yes.accesskit_node().is_disabled());
    yes.click_accesskit();
    harness.run_steps(1);
    assert!(harness.state().state.field_drop_confirmation_open());
    assert!(!harness.state().state.field_drop_dirty());
    assert!(!harness.state().editing.has_save());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert!(!journal_path(&path).exists());
    choose(&mut harness, "field_drop_confirmation", "No");
    assert!(!harness.state().state.field_drop_open());
}

#[test]
fn exact_review_cancellation_preserves_pending_deletion_and_ignores_late_capability() {
    let before = raw::source(raw::SourceCase::NumberHistory);
    let after = raw::remove_nonprimary(&before, "Score", raw::Variant::AppendedGroupRemoved);
    let (_directory, path, _, mut harness) = test_support::opened(before.clone());
    pending(&mut harness, "Score");
    let (request_id, review, capability) = take_review(&mut harness);

    harness.state_mut().cancel_field_drop_review(request_id + 1);
    assert!(harness.state().state.busy);
    assert!(harness.state().editing.field_drop.is_some());
    harness.state_mut().cancel_field_drop_review(request_id);
    assert!(!harness.state().state.busy);
    assert!(harness.state().state.field_drop_dirty());
    assert!(harness.state().state.can_review_field_drop());
    assert!(harness.state().editing.field_drop.is_none());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert!(!journal_path(&path).exists());

    harness
        .state_mut()
        .start_reviewed_field_drop(request_id, review, capability);
    assert!(harness.state().editing.save.is_none());
    assert!(harness.state().state.field_drop_dirty());
    harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
    harness.run_steps(1);
    finish_schema_save(&mut harness);
    assert_eq!(std::fs::read(&path).unwrap(), after);
    assert!(!journal_path(&path).exists());
}

#[test]
fn saved_ack_then_cancelled_and_failed_witness_preserve_old_ui_and_saved_warning() {
    let before = raw::source(raw::SourceCase::NumberHistory);
    let after = raw::remove_nonprimary(&before, "Score", raw::Variant::AppendedGroupRemoved);
    let (_directory, path, _, mut harness) = test_support::opened(before);
    let original = harness.state().state.active_object().unwrap();
    pending(&mut harness, "Score");
    accept_review(&mut harness);
    let identity = harness
        .state()
        .editing
        .field_drop
        .as_ref()
        .unwrap()
        .identity
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while harness.state().editing.worker.poll().is_none() {
        assert!(Instant::now() < deadline, "native DROP did not finish");
        std::thread::yield_now();
    }
    // A real cancellation latch keeps the completed report at the manual ACK
    // boundary. No fabricated outcome or test-only actor pause is involved.
    harness.state_mut().cancel_save(identity.save_id);
    harness.state_mut().poll_saves();
    assert!(matches!(
        harness.state().editing.save_report().unwrap().outcome,
        SaveOutcome::Saved { .. }
    ));
    assert_eq!(std::fs::read(&path).unwrap(), after);
    assert_eq!(harness.state().state.active_object().unwrap(), original);
    assert!(harness.state_mut().acknowledge_field_drop_actor(identity));
    assert!(harness.state().editing.save.is_none());

    harness.state_mut().reopen_field_drop(identity);
    assert!(harness.state().state.busy);
    harness
        .state_mut()
        .field_drop_save_command(identity, FieldDropSaveCommand::Cancel);
    assert!(!harness.state().state.busy);
    assert!(
        !harness
            .state()
            .editing
            .field_drop
            .as_ref()
            .unwrap()
            .reopening
    );
    assert!(harness.state().state.can_retry_field_drop_reopen(identity));
    assert_eq!(harness.state().state.active_object().unwrap(), original);

    let mut drifted = after;
    *drifted.last_mut().unwrap() ^= 1;
    std::fs::write(&path, &drifted).unwrap();
    harness
        .state_mut()
        .field_drop_save_command(identity, FieldDropSaveCommand::RetryReopen);
    finish_reads(&mut harness);
    assert!(harness.state().state.can_retry_field_drop_reopen(identity));
    assert_eq!(harness.state().state.active_object().unwrap(), original);
    assert!(
        harness
            .state()
            .state
            .data
            .as_ref()
            .unwrap()
            .columns
            .iter()
            .any(|field| { field.name == "Score" && field.native_number == Some(1) })
    );
    assert!(harness.state_mut().return_to_field_drop_draft(identity));
    let warning = harness.state().state.field_drop_warning().unwrap();
    assert_eq!(warning.target.column, "Score");
    assert_eq!(warning.target.native_number, 1);
    assert!(matches!(warning.outcome, Some(EditSaveOutcome::Saved(_))));
    assert!(warning.reopen_error.is_some());
    assert!(harness.state().state.native_source_stale());
    assert!(harness.state().state.native_mutation_blocked());
    assert!(harness.state().editing.field_drop.is_none());
    assert!(!harness.state().editing.has_save());
    assert_eq!(std::fs::read(&path).unwrap(), drifted);
    assert!(!journal_path(&path).exists());
}

#[test]
fn source_drift_retains_only_release_then_returns_to_pending_without_original_source_recovery() {
    let before = raw::source(raw::SourceCase::NumberHistory);
    let (_directory, path, _, mut harness) = test_support::opened(before.clone());
    let original = harness.state().state.active_object().unwrap();
    pending(&mut harness, "Score");
    let (request_id, review, capability) = take_review(&mut harness);
    let mut foreign = before;
    foreign[7 * 4096 + 3000] ^= 1;
    std::fs::write(&path, &foreign).unwrap();
    harness
        .state_mut()
        .start_reviewed_field_drop(request_id, review, capability);
    let report = test_support::finish_save(&mut harness);
    let SaveOutcome::Failed(failure) = report.outcome else {
        panic!("fresh-before source drift must refuse the real save");
    };
    assert_eq!(
        failure.kind,
        tablecraft_runtime::SaveFailureKind::SourceChanged
    );
    assert!(failure.can_release);
    assert!(!failure.can_recover);
    assert_eq!(std::fs::read(&path).unwrap(), foreign);
    assert!(!journal_path(&path).exists());
    let identity = harness
        .state()
        .editing
        .field_drop
        .as_ref()
        .unwrap()
        .identity
        .unwrap();
    harness
        .state_mut()
        .field_drop_save_command(identity, FieldDropSaveCommand::Release);
    let released = test_support::finish_save(&mut harness);
    assert!(matches!(released.outcome, SaveOutcome::Released { .. }));
    let released_identity = harness
        .state()
        .editing
        .field_drop
        .as_ref()
        .unwrap()
        .identity
        .unwrap();
    assert!(
        !harness
            .state_mut()
            .acknowledge_field_drop_actor(released_identity)
    );
    assert!(harness.state().editing.field_drop.is_none());
    assert!(!harness.state().editing.has_save());
    assert!(harness.state().state.field_drop_dirty());
    assert_eq!(harness.state().generation, request_id);
    assert_eq!(harness.state().state.active_object().unwrap(), original);
    assert_eq!(std::fs::read(&path).unwrap(), foreign);
    assert!(!journal_path(&path).exists());
}

//! Reuses the actual review/report helpers; cancellation never fabricates AFTER.
use super::*;
use std::sync::Arc;
use tablecraft_ui::FieldAdditionSaveCommand;

#[test]
fn cancelling_acknowledged_after_read_retains_witness_outcome_and_allows_retry() {
    let before = raw::source(raw::Source::IdOnly);
    let after = raw::append(&before, raw::Source::IdOnly, "Score");
    let (_directory, path, _, mut harness) = test_support::opened(before);
    enable(&mut harness);
    let tab = harness.state().state.active_object().unwrap();
    assert!(harness.state_mut().state.begin_field_addition_name(
        &tab,
        "Score".into(),
        FieldAdditionDeclaration::NumberLongZero,
    ));
    let request = harness
        .state()
        .state
        .field_addition_review_request()
        .unwrap();
    harness.state_mut().save_field_addition(request);
    let (generation, review, capability) = take_review(&harness);
    harness
        .state_mut()
        .start_reviewed_field_addition(generation, review, capability);
    let identity = harness
        .state()
        .editing
        .field_addition
        .as_ref()
        .unwrap()
        .identity
        .unwrap();
    let report = take_report(&harness);
    assert!(matches!(report.outcome, SaveOutcome::Saved { .. }));
    // Suppress normal auto-ACK using the real actor cancellation latch, after
    // the terminal save has already completed. No result is substituted.
    harness.state_mut().cancel_save(identity.save_id);
    harness.state_mut().poll_saves();
    assert!(
        harness
            .state_mut()
            .acknowledge_field_addition_actor(identity)
    );
    let host = harness.state().editing.field_addition.as_ref().unwrap();
    let witness = Arc::clone(host.witness.as_ref().unwrap());
    let cleanup = host.cleanup;
    let terminal = host.terminal;
    assert!(harness.state().editing.save.is_none());
    assert_eq!(std::fs::read(&path).unwrap(), after);
    harness.state_mut().reopen_field_addition(identity);
    let read_generation = harness.state().generation;
    assert_ne!(read_generation, identity.request_id);
    assert!(harness.state().state.busy);
    // No intervening frame can consume the read. Even an already completed
    // native read loses publication when its worker request is cancelled.
    harness
        .state_mut()
        .field_addition_save_command(identity, FieldAdditionSaveCommand::Cancel);
    assert!(
        !harness
            .state()
            .worker
            .publication_is_current(read_generation)
    );
    assert!(!harness.state().state.busy);
    let host = harness.state().editing.field_addition.as_ref().unwrap();
    assert!(!host.reopening);
    assert!(Arc::ptr_eq(host.witness.as_ref().unwrap(), &witness));
    assert_eq!(host.terminal, terminal);
    assert_eq!(host.cleanup, cleanup);
    assert!(
        harness
            .state()
            .state
            .can_retry_field_addition_reopen(identity)
    );
    assert!(
        harness
            .state()
            .state
            .can_return_to_field_addition_draft(identity)
    );
    assert_eq!(
        harness.state().state.field_addition_name().unwrap().1,
        "Score"
    );
    assert!(matches!(
        harness
            .state()
            .state
            .field_addition_save_status()
            .unwrap()
            .1,
        Some(EditSaveOutcome::Saved(_))
    ));
    assert_eq!(std::fs::read(&path).unwrap(), after);
    // Retry only the retained full-image witness, never the native write.
    harness.state_mut().reopen_field_addition(identity);
    let deadline = Instant::now() + Duration::from_secs(10);
    while harness.state().state.busy || harness.state().editing.field_addition.is_some() {
        assert!(
            Instant::now() < deadline,
            "cancelled AFTER read did not retry: {:?}",
            harness.state().state.error
        );
        harness.run_steps(1);
        std::thread::yield_now();
    }
    finish_reads(&mut harness);
    assert_eq!(harness.state().state.active_object().unwrap().id, tab.id);
    assert_eq!(
        harness
            .state()
            .state
            .data
            .as_ref()
            .unwrap()
            .columns
            .last()
            .unwrap()
            .name,
        "Score"
    );
    assert_eq!(std::fs::read(&path).unwrap(), after);
    assert!(!journal_path(&path).exists());
}

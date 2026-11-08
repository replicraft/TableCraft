//! Ordinary success advances automatically; retained/cancelled resolution does not.
use super::{
    ordinary_applied_query_save, query, raw_save_report, start_reviewed_save, storage_pair,
};
use crate::workspace::test_support::{finish_reads, opened};
use tablecraft_runtime::{SaveCompletion, SaveOperation, SaveOutcome};
use tablecraft_ui::EditSaveOutcome;

#[test]
fn actual_ordinary_query_receipt_is_distinct_from_recovery_release_and_other_scope() {
    let [before, after] = storage_pair();
    let (_directory, path, _, mut harness) = opened(before);
    query(&mut harness);
    let request = harness
        .state()
        .state
        .saved_query_sql_review_request()
        .unwrap();
    harness.state_mut().save_query_definition(request);
    start_reviewed_save(&mut harness);
    let report = raw_save_report(&harness);
    assert!(ordinary_applied_query_save(report));
    let SaveOutcome::Saved { receipt } = report.outcome else {
        panic!("real native query apply receipt expected");
    };
    let mut recovered = report;
    recovered.operation = SaveOperation::Recover;
    assert!(!ordinary_applied_query_save(recovered));
    let mut confirmed = report;
    if let SaveOutcome::Saved { receipt } = &mut confirmed.outcome {
        receipt.completion = SaveCompletion::Confirmed;
    }
    assert!(!ordinary_applied_query_save(confirmed));
    let mut released = report;
    released.operation = SaveOperation::Release;
    released.outcome = SaveOutcome::Released {
        current_outcome: Some(receipt.known),
        last_completed: Some(receipt),
    };
    assert!(!ordinary_applied_query_save(released));
    let mut other_scope = report;
    if let SaveOutcome::Saved { receipt } = &mut other_scope.outcome {
        receipt.query_change = None;
    }
    assert!(!ordinary_applied_query_save(other_scope));
    // Publish only the genuine retained actor report. The envelopes above are
    // classification controls, never installed as native operation outcomes.
    assert!(
        harness
            .state()
            .editing
            .save
            .as_ref()
            .unwrap()
            .report
            .is_none()
    );
    harness.state_mut().poll_saves();
    assert!(harness.state().editing.save.is_none());
    assert!(harness.state().state.saved_query_sql_dirty());
    finish_reads(&mut harness);
    assert!(!harness.state().state.saved_query_sql_dirty());
    assert_eq!(std::fs::read(path).unwrap(), after);
}

#[test]
fn late_cancel_keeps_actual_saved_fact_and_requires_explicit_acknowledgement() {
    let [before, after] = storage_pair();
    let (_directory, path, _, mut harness) = opened(before);
    query(&mut harness);
    let request = harness
        .state()
        .state
        .saved_query_sql_review_request()
        .unwrap();
    harness.state_mut().save_query_definition(request);
    start_reviewed_save(&mut harness);
    let report = raw_save_report(&harness);
    assert!(matches!(report.outcome, SaveOutcome::Saved { .. }));
    // Request cancellation after actual completion but before desktop report
    // publication. This deterministic boundary cannot turn Saved into rollback.
    harness.state_mut().cancel_current_save();
    harness.state_mut().poll_saves();
    assert!(harness.state().editing.save.is_some());
    let (identity, outcome) = harness.state().state.saved_query_sql_save_status().unwrap();
    assert!(matches!(outcome, Some(EditSaveOutcome::Saved(_))));
    assert!(
        harness
            .state()
            .state
            .can_acknowledge_saved_query_sql_save(identity)
    );
    assert!(harness.state().state.saved_query_sql_dirty());
    assert_eq!(std::fs::read(&path).unwrap(), after);
    harness.state_mut().acknowledge_query_definition(report.id);
    assert!(harness.state().state.saved_query_sql_dirty());
    finish_reads(&mut harness);
    assert!(!harness.state().editing.has_save());
    assert!(!harness.state().state.saved_query_sql_dirty());
    assert_eq!(std::fs::read(path).unwrap(), after);
}

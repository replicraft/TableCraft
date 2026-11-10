//! Real read/save actors at the rename-specific publication boundaries.
mod close;
mod current_storage;
mod design;
mod preferences;
use super::*;
use crate::workspace::test_support::{self, finish_reads, finish_schema_save};
use egui_kittest::Harness;
use std::time::{Duration, Instant};
use tablecraft_runtime::{Response, SaveOutcome, SaveWorker};
use tablecraft_testkit::fixtures::access::{
    canonical_created_number::mixed::after_off::SourceCase, saved_field_rename,
};
use tablecraft_ui::{EditSaveOutcome, FieldRenameTarget};

fn inspected(harness: &mut Harness<'_, Desktop>, column: &str) -> FieldRenameTarget {
    // The ordinary host supplies only platform support; the following real
    // source inspection and named review still qualify every native request.
    assert_eq!(
        harness.state().state.field_rename_available,
        SaveWorker::supports_field_rename()
    );
    assert!(harness.state().state.field_rename_available);
    let tab = harness.state().state.active_object().unwrap();
    let native_number = harness
        .state()
        .state
        .data
        .as_ref()
        .unwrap()
        .columns
        .iter()
        .find(|candidate| candidate.name == column)
        .unwrap()
        .native_number
        .unwrap();
    let target = FieldRenameTarget {
        tab,
        column: column.into(),
        native_number,
    };
    assert!(
        harness
            .state_mut()
            .state
            .begin_field_rename(FieldRenameTarget {
                tab: target.tab.clone(),
                column: target.column.clone(),
                native_number,
            })
    );
    let outgoing = harness
        .state()
        .state
        .field_rename_inspection_target()
        .unwrap();
    harness.state_mut().inspect_field_rename_source(outgoing);
    finish_reads(harness);
    assert!(harness.state().state.can_edit_field_rename());
    target
}

fn accept_review(harness: &mut Harness<'_, Desktop>, new_name: &str) {
    let target = inspected(harness, "Score");
    assert!(
        harness
            .state_mut()
            .state
            .set_field_rename_name(&target, new_name.into())
    );
    let request = harness.state().state.field_rename_review_request().unwrap();
    harness.state_mut().save_field_rename(request);
    let deadline = Instant::now() + Duration::from_secs(10);
    let (generation, response) = loop {
        if let Some(reply) = harness.state().worker.poll() {
            break reply;
        }
        assert!(Instant::now() < deadline, "rename review did not finish");
        std::thread::yield_now();
    };
    let Response::FieldRenameReviewed { review, capability } = response.unwrap() else {
        panic!("expected genuine native rename review");
    };
    harness
        .state_mut()
        .start_reviewed_field_rename(generation, review, capability);
}

#[test]
fn unchanged_review_owns_only_a_real_read_witness_and_never_a_native_actor() {
    let [before, _] = saved_field_rename::pair(SourceCase::Number);
    let (_directory, path, _, mut harness) = test_support::opened(before.clone());
    accept_review(&mut harness, "Score");
    let host = harness.state().editing.field_rename.as_ref().unwrap();
    assert!(matches!(
        host.identity,
        Some(FieldRenameSaveIdentity::UnchangedReview { .. })
    ));
    assert!(host.terminal.is_none());
    assert!(host.reopening);
    assert!(harness.state().editing.save.is_none());
    assert!(!harness.state().editing.worker.is_busy());
    assert!(!journal_path(&path).exists());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    finish_schema_save(&mut harness);
    assert!(!harness.state().state.field_rename_open());
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[test]
fn actual_ack_then_failed_witness_can_return_with_saved_facts_and_block_stale_writes() {
    let [before, after] = saved_field_rename::pair(SourceCase::Number);
    let (_directory, path, _, mut harness) = test_support::opened(before);
    accept_review(&mut harness, "Total");
    let identity = harness
        .state()
        .editing
        .field_rename
        .as_ref()
        .unwrap()
        .identity
        .unwrap();
    let FieldRenameSaveIdentity::NativeSave { save_id, .. } = identity else {
        panic!("changed review must own a real actor");
    };
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if harness.state().editing.worker.poll().is_some() {
            break;
        }
        assert!(Instant::now() < deadline, "native rename did not finish");
        std::thread::yield_now();
    }
    // A real cancellation latch prevents automatic ACK of the actual completed
    // report. Poll the real retained mailbox, then cross the production ACK
    // boundary before changing bytes; no fake receipt or paused test mode.
    harness.state_mut().cancel_save(save_id);
    harness.state_mut().poll_saves();
    assert!(matches!(
        harness.state().editing.save_report().unwrap().outcome,
        SaveOutcome::Saved { .. }
    ));
    assert_eq!(std::fs::read(&path).unwrap(), after);
    assert!(harness.state_mut().acknowledge_field_rename_actor(identity));
    assert!(harness.state().editing.save.is_none());
    let mut drifted = after.clone();
    *drifted.last_mut().unwrap() ^= 1;
    std::fs::write(&path, &drifted).unwrap();
    harness.state_mut().reopen_field_rename(identity);
    let deadline = Instant::now() + Duration::from_secs(10);
    while harness.state().state.busy {
        assert!(Instant::now() < deadline, "guarded reopen did not finish");
        harness.run_steps(1);
        std::thread::yield_now();
    }
    assert!(
        harness
            .state()
            .state
            .can_return_to_field_rename_draft(identity)
    );
    assert!(harness.state_mut().return_to_field_rename_draft(identity));
    let warning = harness.state().state.field_rename_warning().unwrap();
    assert!(matches!(warning.outcome, Some(EditSaveOutcome::Saved(_))));
    assert!(harness.state().state.native_source_stale());
    assert!(harness.state().state.native_mutation_blocked());
    assert_eq!(
        harness.state().state.field_rename_proposal().unwrap().1,
        "Total"
    );
    assert_eq!(std::fs::read(&path).unwrap(), drifted);
    assert!(!journal_path(&path).exists());
}

mod fitting;

mod populated;

mod repeated;

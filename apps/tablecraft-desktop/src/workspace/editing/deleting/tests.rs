//! Actual row selection, confirmation and shared save-owner routing without windows.
use super::*;
use crate::workspace::test_support::{self, finish_reads, finish_save};
use egui::accesskit::Role;
use egui_kittest::{Harness, kittest::Queryable};
use tablecraft_runtime::{SaveFailureKind, SaveOutcome};
use tablecraft_testkit::fixtures::access::{canonical_creation, canonical_native17_creation};
use tablecraft_testkit::support::access::retained_target::assert_retained_target_observation;

fn source() -> Vec<u8> {
    canonical_native17_creation::source(
        canonical_creation::source(canonical_creation::Case::Extrema).bytes,
    )
}

fn review_second(harness: &mut Harness<'_, Desktop>) {
    harness
        .get_all_by_role(Role::RowHeader)
        .nth(1)
        .unwrap()
        .click_accesskit();
    harness.run_steps(2);
    assert_eq!(
        harness
            .state()
            .state
            .current_delete_target()
            .map(|target| target.parent_ordinal),
        Some(1),
    );
    harness
        .get_by_role_and_label(Role::Button, "Delete")
        .click_accesskit();
    harness.run_steps(2);
    assert!(harness.state().editing.delete_review.is_none());
    harness
        .get_by_role_and_label(Role::Button, "Delete row")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(harness);
    assert!(harness.state().editing.delete_review.is_some());
}

#[test]
fn confirmed_delete_refreshes_after_ack_and_cancel_or_stale_target_never_writes() {
    let (_directory, path, original, mut harness) = test_support::opened(source());
    assert_eq!(harness.state().state.data.as_ref().unwrap().rows.len(), 2);
    review_second(&mut harness);
    let cancelled = harness.state().editing.delete_review.as_ref().unwrap().0;
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert!(!journal_path(&path).exists());
    harness
        .get_by_role_and_label(Role::Button, "Cancel")
        .click_accesskit();
    harness.run_steps(2);
    harness.state_mut().apply_delete(cancelled);
    assert!(!harness.state().editing.has_save());
    assert!(harness.state().editing.delete_review.is_none());
    assert_eq!(std::fs::read(&path).unwrap(), original);

    let generation = harness.state().generation;
    let revision = harness
        .state()
        .state
        .database
        .as_ref()
        .unwrap()
        .document_revision;
    harness
        .state_mut()
        .review_delete(tablecraft_ui::DeleteTarget {
            document_revision: revision + 1,
            table: "Items".into(),
            parent_ordinal: 1,
        });
    assert_eq!(harness.state().generation, generation);
    assert!(harness.state().editing.delete_review.is_none());

    review_second(&mut harness);
    harness
        .get_by_role_and_label(Role::Button, "Delete reviewed row")
        .click_accesskit();
    harness.run_steps(1);
    let report = finish_save(&mut harness);
    assert!(
        matches!(report.outcome, SaveOutcome::Saved { receipt } if receipt.changed_rows == Some(1))
    );
    assert!(matches!(
        harness.state().editing.save.as_ref().unwrap().owner,
        SaveOwner::Delete
    ));
    assert_eq!(harness.state().state.data.as_ref().unwrap().rows.len(), 2);
    assert_eq!(harness.state().editing.worker.poll(), Some(report));
    harness
        .get_by_role_and_label(Role::Button, "Close")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    assert!(!harness.state().editing.has_save());
    let data = harness.state().state.data.as_ref().unwrap();
    assert_eq!(data.table, "Items");
    assert_eq!(data.rows.len(), 1);
    assert_eq!(data.rows[0][0].text, "1");
    assert_eq!(data.rows[0][1].text, "-2147483648");
    assert!(!journal_path(&path).exists());
}

#[test]
fn source_drift_refuses_delete_and_keeps_owner_until_explicit_release_and_ack() {
    let (_directory, path, original, mut harness) = test_support::opened(source());
    review_second(&mut harness);
    let mut changed = original.clone();
    *changed.last_mut().unwrap() ^= 1;
    std::fs::write(&path, &changed).unwrap();
    harness
        .get_by_role_and_label(Role::Button, "Delete reviewed row")
        .click_accesskit();
    harness.run_steps(1);
    let report = finish_save(&mut harness);
    assert!(matches!(report.outcome, SaveOutcome::Failed(failure)
        if failure.kind == SaveFailureKind::SourceChanged && failure.can_release && !failure.can_recover));
    assert!(harness.state().editing.has_save());
    assert!(!harness.state().editing.worker.acknowledge(report.id));
    assert_retained_target_observation(&path, &changed);
    assert!(!journal_path(&path).exists());

    harness
        .get_by_role_and_label(Role::Button, "Release save resources")
        .click_accesskit();
    harness.run_steps(1);
    let released = finish_save(&mut harness);
    assert!(matches!(
        released.outcome,
        SaveOutcome::Released {
            current_outcome: None,
            last_completed: None
        }
    ));
    assert_ne!(released.id, report.id);
    assert!(harness.state().editing.has_save());
    harness
        .get_by_role_and_label(Role::Button, "Close")
        .click_accesskit();
    harness.run_steps(2);
    assert!(!harness.state().editing.has_save());
    assert!(harness.state().state.database.is_none());
    assert_eq!(std::fs::read(&path).unwrap(), changed);
    assert!(!journal_path(&path).exists());
}

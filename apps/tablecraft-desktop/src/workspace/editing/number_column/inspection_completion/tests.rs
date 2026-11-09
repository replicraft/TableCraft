//! Actual readonly results resolve recorded parked owners after generation replacement.
use crate::workspace::{
    Desktop, PendingOperation,
    test_support::{self, finish_reads},
};
use egui::accesskit::Role;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use std::time::{Duration, Instant};
use tablecraft_runtime::{Command, Response};
use tablecraft_testkit::fixtures::access::canonical_created_number as raw;
use tablecraft_ui::{ObjectKind, ObjectTab, ObjectTarget};

// Normal startup now completes the first source inspection. Ask the actual
// typed retry controller for a fresh publication to exercise cancellation;
// never clear cache state or manufacture a pending/busy owner in the test.
fn request_fresh_inspection(harness: &mut Harness<'_, Desktop>) -> ObjectTab {
    assert!(harness.state().state.number_column_available);
    let owner = harness.state().state.active_object().unwrap();
    assert!(
        !harness
            .state()
            .state
            .number_column_inspection(&owner)
            .unwrap()
            .0
    );
    harness
        .state_mut()
        .retry_number_column_availability(owner.clone());
    assert!(harness.state().state.busy);
    assert!(
        harness
            .state()
            .state
            .has_pending_number_column_inspection(harness.state().generation)
    );
    owner
}

#[test]
fn completed_native_inspection_is_interrupted_for_parked_owner_when_publication_is_replaced() {
    let before = raw::source(raw::Source::IdOnly);
    let (_directory, path, _, mut harness) = test_support::opened(before.clone());
    let owner = request_fresh_inspection(&mut harness);
    let deadline = Instant::now() + Duration::from_secs(10);
    let (old_generation, result) = loop {
        if let Some(result) = harness.state_mut().worker.poll() {
            break result;
        }
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    };
    // The real readonly task has finished, but its result has not been published.
    // Release its presentation busy flag and park that exact owner before the
    // observed terminal response enters the stale-generation completion path.
    harness.state_mut().state.busy = false;
    let scratch = harness
        .state_mut()
        .state
        .prepare_object_open(ObjectTarget {
            document_revision: owner.target.document_revision,
            kind: ObjectKind::Sql,
        })
        .unwrap()
        .tab;
    let exact = " SELECT 7; -- keep scratch\n";
    assert!(harness.state_mut().state.replace_query_sql(exact.into()));
    harness.state_mut().submit(Command::Page {
        table: "Table1".into(),
        offset: 0,
    });
    let current_generation = harness.state().generation;
    assert_ne!(old_generation, current_generation);
    harness
        .state_mut()
        .finish_superseded_number_inspection(old_generation, result);
    assert_eq!(harness.state().generation, current_generation);
    assert!(matches!(harness.state().pending, PendingOperation::Read));
    assert!(harness.state().state.busy);
    assert_eq!(harness.state().state.active_object(), Some(scratch));
    assert_eq!(harness.state().state.sql, exact);
    assert_eq!(
        harness
            .state()
            .state
            .number_column_inspection(&owner)
            .unwrap()
            .1,
        None
    );
    assert!(
        !harness
            .state()
            .state
            .has_pending_number_column_inspection(old_generation)
    );
    assert!(
        harness
            .state()
            .state
            .number_column_inspection_interrupted(&owner)
    );
    assert!(harness.state().editing.save.is_none());
    assert_eq!(std::fs::read(path).unwrap(), before);
}

#[test]
fn normally_superseded_native_inspection_is_terminal_without_restarting_proof() {
    let before = raw::source(raw::Source::IdOnly);
    let (_directory, path, _, mut harness) = test_support::opened(before.clone());
    let owner = request_fresh_inspection(&mut harness);
    let old_generation = harness.state().generation;
    harness.state_mut().submit(Command::Page {
        table: "Table1".into(),
        offset: 0,
    });
    let next = harness.state().generation;
    finish_reads(&mut harness);
    assert_eq!(harness.state().generation, next);
    assert!(
        !harness
            .state()
            .state
            .has_pending_number_column_inspection(old_generation)
    );
    assert!(
        !harness
            .state()
            .state
            .number_column_inspection(&owner)
            .unwrap()
            .0
    );
    assert!(
        harness
            .state()
            .state
            .number_column_inspection_interrupted(&owner)
    );
    assert!(harness.state().editing.save.is_none());
    assert_eq!(std::fs::read(path).unwrap(), before);
}

#[test]
fn escape_cancel_retires_the_readonly_publication_and_retry_uses_the_actual_button() {
    let before = raw::source(raw::Source::IdOnly);
    let (_directory, path, _, mut harness) = test_support::opened(before.clone());
    let owner = request_fresh_inspection(&mut harness);
    let old = harness.state().generation;
    let deadline = Instant::now() + Duration::from_secs(10);
    let (finished_id, completed_reply) = loop {
        if let Some(reply) = harness.state_mut().worker.poll() {
            break reply;
        }
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    };
    assert_eq!(finished_id, old);
    assert!(
        matches!(&completed_reply, Ok(Response::NumberColumnAvailability { availability }) if availability.available)
    );
    // Hold a real completed reply before ordinary publication so Escape sees
    // the actual busy presentation deterministically. Only the production
    // shortcut/Action::Cancel path may revoke publication and clear busy.
    assert!(harness.state().state.busy);
    harness.key_press(egui::Key::Escape);
    harness.run_steps(1);
    assert!(!harness.state().state.busy);
    assert!(!harness.state().worker.publication_is_current(old));
    // The following normal logic frame retires the revoked inspection slot.
    harness.run_steps(1);
    drop(completed_reply);
    assert!(!harness.state().worker.publication_is_current(old));
    assert!(
        !harness
            .state()
            .state
            .has_pending_number_column_inspection(old)
    );
    assert!(
        harness
            .state()
            .state
            .number_column_inspection_interrupted(&owner)
    );
    assert!(!harness.state().state.needs_number_column_inspection(&owner));
    assert!(!harness.state().state.can_name_number_column(&owner));
    let retry_label = harness
        .state()
        .state
        .localizer
        .text("datasheet-number-retry-inspection");
    let retry = harness.get_by_role_and_label(Role::Button, &retry_label);
    assert!(!retry.accesskit_node().is_disabled());
    retry.click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    assert_ne!(harness.state().generation, old);
    assert!(harness.state().state.can_name_number_column(&owner));
    assert!(
        !harness
            .state()
            .state
            .number_column_inspection_interrupted(&owner)
    );
    assert_eq!(std::fs::read(path).unwrap(), before);
}

//! Headless desktop acceptance uses real native reads, saves and full-image witnesses.
use crate::workspace::{
    Desktop,
    test_support::{self, finish_reads, finish_schema_save},
};
use egui::{Id, accesskit::Role};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use std::time::{Duration, Instant};
use tablecraft_runtime::{
    Command, Response, ReviewedTableDrop, SaveOutcome, TableDropReview, journal_path,
};
use tablecraft_testkit::fixtures::access::canonical_table_drop as raw;
use tablecraft_ui::{ObjectTarget, TableDropSaveCommand};

fn closed(harness: &mut Harness<'_, Desktop>) -> ObjectTarget {
    let tab = harness.state().state.active_object().unwrap();
    let target = tab.target.clone();
    harness.state_mut().close_object(tab);
    finish_reads(harness);
    assert!(harness.state().state.object_tabs().is_empty());
    target
}

fn inspected(harness: &mut Harness<'_, Desktop>, target: ObjectTarget) {
    assert!(harness.state_mut().state.begin_table_drop(target));
    harness.run_steps(1);
    finish_reads(harness);
    assert!(harness.state().state.table_drop_open());
}

fn choose(harness: &mut Harness<'_, Desktop>, label: &str) {
    let dialog = harness
        .ctx
        .memory(|memory| memory.area_rect(Id::new("table_drop")))
        .unwrap();
    let buttons = harness
        .get_all_by_role_and_label(Role::Button, label)
        .filter(|button| dialog.contains_rect(button.rect()))
        .collect::<Vec<_>>();
    assert_eq!(buttons.len(), 1);
    assert!(!buttons[0].accesskit_node().is_disabled());
    buttons[0].click_accesskit();
    drop(buttons);
    harness.run_steps(1);
}

fn take_review(harness: &mut Harness<'_, Desktop>) -> (u64, TableDropReview, ReviewedTableDrop) {
    choose(harness, "Yes");
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some((generation, result)) = harness.state().worker.poll() {
            let Response::TableDropReviewed { review, capability } = result.unwrap() else {
                panic!("expected actual source-bound table DROP review");
            };
            return (generation, review, capability);
        }
        assert!(
            Instant::now() < deadline,
            "native table DROP review stalled"
        );
        std::thread::yield_now();
    }
}

#[test]
fn yes_immediately_saves_empty_table_and_publishes_only_checked_reopened_catalog() {
    for header in [5, 6] {
        let pair = raw::unindexed(header, "Items", false);
        let (_directory, path, _, mut harness) = test_support::opened(pair.before.clone());
        let target = closed(&mut harness);
        let revision = target.document_revision;
        inspected(&mut harness, target);
        assert!(
            harness
                .get_by_role_and_label(Role::Button, "Yes")
                .is_focused()
        );
        assert_eq!(std::fs::read(&path).unwrap(), pair.before);
        assert!(!journal_path(&path).exists());
        choose(&mut harness, "Yes");
        finish_schema_save(&mut harness);
        let database = harness.state().state.database.as_ref().unwrap();
        assert!(database.document_revision > revision);
        assert!(database.tables.iter().all(|table| table.system));
        assert!(database.tables.iter().all(|table| table.name != pair.table));
        assert!(harness.state().state.object_tabs().is_empty());
        assert!(!harness.state().state.table_drop_open());
        assert!(harness.state().editing.table_drop.is_none());
        assert!(!harness.state().editing.worker.is_busy());
        assert_eq!(std::fs::read(&path).unwrap(), pair.after);
        assert!(!journal_path(&path).exists());
    }
}

#[test]
fn populated_source_disables_yes_and_no_preserves_native_file_without_save_owner() {
    let pair = raw::unindexed(5, "Items", true);
    let (_directory, path, _, mut harness) = test_support::opened(pair.before.clone());
    let target = closed(&mut harness);
    inspected(&mut harness, target);
    assert!(
        harness
            .get_by_role_and_label(Role::Button, "Yes")
            .accesskit_node()
            .is_disabled()
    );
    assert!(harness.state().editing.save.is_none());
    choose(&mut harness, "No");
    finish_reads(&mut harness);
    assert!(!harness.state().state.table_drop_open());
    assert!(!harness.state().editing.has_save());
    assert_eq!(std::fs::read(&path).unwrap(), pair.before);
    assert!(!journal_path(&path).exists());
}

#[test]
fn real_review_without_ui_owner_and_mismatched_catalog_identity_never_start_native_save() {
    let pair = raw::unindexed(5, "Items", false);
    let (_directory, path, _, mut harness) = test_support::opened(pair.before.clone());
    let target = closed(&mut harness);
    harness.state_mut().submit(Command::ReviewTableDrop {
        document_revision: target.document_revision,
        table: "Items".into(),
    });
    finish_reads(&mut harness);
    assert!(!harness.state().editing.has_save());
    inspected(&mut harness, target);
    let (generation, mut review, capability) = take_review(&mut harness);
    review.native_id += 1;
    harness
        .state_mut()
        .start_reviewed_table_drop(generation, review, capability);
    assert!(harness.state().editing.table_drop.is_none());
    assert!(harness.state().editing.save.is_none());
    assert!(!harness.state().editing.worker.is_busy());
    assert_eq!(std::fs::read(&path).unwrap(), pair.before);
    assert!(!journal_path(&path).exists());
}

#[test]
fn tampered_after_keeps_old_catalog_until_retry_checks_the_same_complete_witness() {
    let pair = raw::unindexed(6, "Items", false);
    let (_directory, path, _, mut harness) = test_support::opened(pair.before.clone());
    let target = closed(&mut harness);
    let revision = target.document_revision;
    inspected(&mut harness, target);
    let (generation, review, capability) = take_review(&mut harness);
    harness
        .state_mut()
        .start_reviewed_table_drop(generation, review, capability);
    let identity = harness
        .state()
        .editing
        .table_drop
        .as_ref()
        .unwrap()
        .save
        .identity
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(report) = harness.state().editing.worker.poll() {
            assert!(matches!(report.outcome, SaveOutcome::Saved { .. }));
            break;
        }
        assert!(Instant::now() < deadline, "native table DROP save stalled");
        std::thread::yield_now();
    }
    let mut foreign = pair.after.clone();
    *foreign.last_mut().unwrap() ^= 1;
    std::fs::write(&path, &foreign).unwrap();
    while harness.state().state.busy {
        assert!(Instant::now() < deadline, "guarded reopen did not settle");
        harness.run_steps(1);
        std::thread::yield_now();
    }
    let database = harness.state().state.database.as_ref().unwrap();
    assert_eq!(database.document_revision, revision);
    let user_tables = database
        .tables
        .iter()
        .filter(|table| !table.system)
        .collect::<Vec<_>>();
    assert_eq!(user_tables.len(), 1);
    assert_eq!(user_tables[0].name, "Items");
    assert!(harness.state().state.can_retry_table_drop_reopen(identity));
    assert!(harness.state().editing.table_drop.is_some());
    assert!(!journal_path(&path).exists());
    assert_eq!(std::fs::read(&path).unwrap(), foreign);

    std::fs::write(&path, &pair.after).unwrap();
    harness
        .state_mut()
        .table_drop_save_command(identity, TableDropSaveCommand::RetryReopen);
    finish_schema_save(&mut harness);
    assert!(
        harness
            .state()
            .state
            .database
            .as_ref()
            .unwrap()
            .tables
            .iter()
            .all(|table| table.system)
    );
    assert!(!harness.state().state.table_drop_open());
    assert!(harness.state().editing.table_drop.is_none());
    assert_eq!(std::fs::read(path).unwrap(), pair.after);
}

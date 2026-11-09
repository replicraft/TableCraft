//! Command-only ownership negatives, separate from actual widget acceptance.
use crate::workspace::test_support::{self, finish_reads};
use tablecraft_runtime::{Command, journal_path};
use tablecraft_testkit::fixtures::access::canonical_created_number::mixed::after_off::deleted_field as raw;

#[test]
fn actual_unlocked_runtime_review_without_confirmed_ui_owner_never_starts_a_save() {
    let before = raw::source(raw::SourceCase::NumberHistory);
    let (_directory, path, _, mut harness) = test_support::opened(before.clone());
    let revision = harness
        .state()
        .state
        .database
        .as_ref()
        .unwrap()
        .document_revision;
    assert!(!harness.state().state.field_drop_open());
    harness.state_mut().submit(Command::ReviewFieldDrop {
        document_revision: revision,
        table: "Table1".into(),
        column: "Score".into(),
        native_number: 1,
    });
    finish_reads(&mut harness);
    assert!(harness.state().editing.field_drop.is_none());
    assert!(harness.state().editing.save.is_none());
    assert!(!harness.state().editing.worker.is_busy());
    assert!(!harness.state().state.field_drop_open());
    assert!(!harness.state().state.field_drop_dirty());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert!(!journal_path(&path).exists());
    assert_eq!(
        harness
            .state()
            .state
            .database
            .as_ref()
            .unwrap()
            .document_revision,
        revision
    );
    assert_eq!(
        harness.state().state.data.as_ref().unwrap().columns.len(),
        3
    );
}

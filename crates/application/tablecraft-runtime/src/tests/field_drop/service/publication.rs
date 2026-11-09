//! Native completion is distinct from replacement of the current read session.
use super::{opened, request, review, terminal};
use crate::{Command, Response, SaveOutcome, SaveWorker};
use tablecraft_testkit::fixtures::access::canonical_created_number::mixed::after_off::deleted_field as raw;

fn current_columns(worker: &crate::Worker) -> (u64, Vec<String>) {
    let Response::Data {
        source_revision,
        columns,
        rows,
        ..
    } = request(
        worker,
        Command::Page {
            table: "Table1".into(),
            offset: 0,
        },
    )
    .unwrap()
    else {
        panic!("expected an actual current-session table page");
    };
    assert!(rows.is_empty());
    (
        source_revision.unwrap(),
        columns.into_iter().map(|column| column.name).collect(),
    )
}

#[test]
fn real_terminal_report_and_native_ack_do_not_publish_a_new_read_session_before_full_witness() {
    let before = raw::source(raw::SourceCase::NumberHistory);
    let after = raw::remove_nonprimary(&before, "Score", raw::Variant::AppendedGroupRemoved);
    let (directory, path, worker, revision) = opened(&before);
    let (_, capability) = review(&worker, revision, "Score", 1).unwrap();
    let witness = capability.reopen_witness();
    let saves = SaveWorker::new(|| {});
    let bundle = directory.path().join("publication.journal");
    let id = saves.start_field_drop(capability, bundle.clone()).unwrap();
    assert!(matches!(
        terminal(&saves).outcome,
        SaveOutcome::Saved { .. }
    ));
    let (current_revision, columns) = current_columns(&worker);
    assert_eq!(current_revision, revision);
    assert_eq!(columns, ["ID", "Score", "Score2"]);
    assert!(saves.acknowledge(id));
    assert!(!bundle.exists());
    assert_eq!(std::fs::read(&path).unwrap(), after);
    let (current_revision, columns) = current_columns(&worker);
    assert_eq!(current_revision, revision);
    assert_eq!(columns, ["ID", "Score", "Score2"]);

    let Response::Opened {
        document_revision: fresh,
        ..
    } = request(&worker, Command::OpenFieldDropped { witness }).unwrap()
    else {
        panic!("only the checked complete-image witness can publish replacement");
    };
    assert_ne!(fresh, revision);
    let (current_revision, columns) = current_columns(&worker);
    assert_eq!(current_revision, fresh);
    assert_eq!(columns, ["ID", "Score2"]);
    assert_eq!(std::fs::read(path).unwrap(), after);
}

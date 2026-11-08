//! Target-specific capacity remains native review authority, separate from UI facts.
use super::{opened_source, request, terminal};
use crate::{Command, Error, Response, SaveOutcome, SaveWorker, journal_path};
use tablecraft_ace::AddTableError;
use tablecraft_builders::table::{DraftFieldType, TableDraft};
use tablecraft_testkit::fixtures::access::canonical_mixed_add_table;

#[test]
fn keyless_only_capacity_rejects_a_key_then_saves_the_corrected_exact_snapshot() {
    let source = canonical_mixed_add_table::capacity_source();
    let (_directory, path, reader, revision) = opened_source(&source.bytes);
    let Response::Opened {
        table_creation_available,
        table_design_available,
        document_revision,
        ..
    } = request(
        &reader,
        Command::Open {
            path: path.clone(),
            password: String::new(),
        },
    )
    .unwrap()
    else {
        panic!("expected a readable keyless-only-capacity source");
    };
    assert_ne!(document_revision, revision);
    assert!(!table_creation_available);
    assert!(table_design_available);

    let mut draft = TableDraft::design();
    let name = draft.add_field(DraftFieldType::ShortText).unwrap();
    draft.rename_field(name, "Name").unwrap();
    let key = draft.add_field(DraftFieldType::Number).unwrap();
    draft.rename_field(key, "ActualKey").unwrap();
    draft.set_primary_key(Some(key)).unwrap();
    let original = draft.snapshot("Last").unwrap();
    assert!(matches!(
        request(
            &reader,
            Command::ReviewTableCreation {
                document_revision,
                snapshot: draft.snapshot("Last").unwrap(),
            }
        ),
        Err(Error::AddTable(AddTableError::Capacity))
    ));
    assert!(draft.snapshot("Last").unwrap() == original);
    assert_eq!(std::fs::read(&path).unwrap(), source.bytes);
    assert!(!journal_path(&path).exists());

    draft.set_primary_key(None).unwrap();
    let Response::TableCreationReviewed { capability } = request(
        &reader,
        Command::ReviewTableCreation {
            document_revision,
            snapshot: draft.snapshot("Last").unwrap(),
        },
    )
    .unwrap() else {
        panic!("expected exact keyless declaration review");
    };
    let witness = capability.reopen_witness();
    let saver = SaveWorker::new(|| {});
    let operation = saver
        .start_table_creation(capability, journal_path(&path))
        .unwrap();
    let outcome = terminal(&saver).outcome;
    assert!(matches!(outcome, SaveOutcome::Saved { .. }), "{outcome:?}");
    assert!(saver.acknowledge(operation));
    let Response::Opened {
        table_creation_available,
        table_design_available,
        ..
    } = request(&reader, Command::OpenTableCreated { witness }).unwrap()
    else {
        panic!("expected exact saved keyless schema reopening");
    };
    assert!(!table_creation_available);
    assert!(!table_design_available);
    assert!(!journal_path(&path).exists());
}

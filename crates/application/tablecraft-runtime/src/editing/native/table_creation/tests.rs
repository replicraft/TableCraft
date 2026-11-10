use crate::tests::{request, terminal};
use crate::{
    Command, Error, Response, SaveKnownOutcome, SaveOutcome, SaveSchemaChange, SaveStartFailure,
    SaveWorker, Worker, journal_path,
};
use std::path::PathBuf;
use tablecraft_builders::table::{DraftFieldType, TableDraft};
use tablecraft_testkit::fixtures::access::canonical_blank_creation;

mod addition;
mod capacity;
mod fixed_scalars;
mod manual;
mod number_source;
mod properties;
mod retained_gap;
mod wide;

fn opened() -> (tempfile::TempDir, PathBuf, Worker, u64) {
    opened_source(&canonical_blank_creation::source())
}

fn opened_source(bytes: &[u8]) -> (tempfile::TempDir, PathBuf, Worker, u64) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("new.accdb");
    std::fs::write(&path, bytes).unwrap();
    let worker = Worker::new(|| {});
    let Response::Opened {
        document_revision, ..
    } = request(
        &worker,
        Command::Open {
            path: path.clone(),
            password: String::new(),
        },
    )
    .unwrap()
    else {
        panic!("expected an opened canonical blank");
    };
    (directory, path, worker, document_revision)
}

fn reviewed(worker: &Worker, document_revision: u64, table: &str) -> crate::ReviewedTableCreation {
    let mut draft = TableDraft::initial();
    draft.add_field(DraftFieldType::ShortText).unwrap();
    let snapshot = draft.snapshot(table).unwrap();
    let Response::TableCreationReviewed { capability } = request(
        worker,
        Command::ReviewTableCreation {
            document_revision,
            snapshot,
        },
    )
    .unwrap() else {
        panic!("expected a source-bound first-table review");
    };
    capability
}

#[test]
fn actual_schema_save_has_no_row_counts_and_reopens_only_the_complete_expected_image() {
    let (_directory, path, reader, document_revision) = opened();
    let original = std::fs::read(&path).unwrap();
    let capability = reviewed(&reader, document_revision, "Contacts");
    let witness = capability.reopen_witness();
    assert_eq!(
        witness.expected_schema_change(),
        SaveSchemaChange::FirstTableCreated { column_count: 2 }
    );
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert!(!journal_path(&path).exists());
    assert_eq!(witness.table_name(), "Contacts");
    assert_eq!(witness.column_count(), 2);

    let saver = SaveWorker::new(|| {});
    let operation = saver
        .start_table_creation(capability, journal_path(&path))
        .unwrap();
    let report = terminal(&saver);
    assert_eq!(report.id, operation);
    let SaveOutcome::Saved { receipt } = report.outcome else {
        panic!("schema save failed: {report:?}");
    };
    assert_eq!(receipt.known, SaveKnownOutcome::Committed);
    assert_eq!(receipt.matched_rows, None);
    assert_eq!(receipt.changed_rows, None);
    assert_eq!(
        receipt.schema_change,
        Some(SaveSchemaChange::FirstTableCreated { column_count: 2 })
    );
    assert_eq!(saver.poll(), Some(report));
    assert!(saver.is_busy());
    assert!(saver.acknowledge(operation));
    assert!(!journal_path(&path).exists());
    assert_eq!(std::fs::read(&path).unwrap(), witness.expected_bytes());

    let Response::Opened {
        document_revision: fresh_revision,
        tables,
        ..
    } = request(&reader, Command::OpenTableCreated { witness }).unwrap()
    else {
        panic!("expected guarded reopening");
    };
    assert_ne!(fresh_revision, document_revision);
    assert!(
        tables
            .iter()
            .any(|table| table.name == "Contacts" && table.local && !table.system)
    );
    let Response::Data { columns, rows, .. } = request(
        &reader,
        Command::Page {
            table: "Contacts".into(),
            offset: 0,
        },
    )
    .unwrap() else {
        panic!("expected a real persisted table page");
    };
    assert_eq!(
        columns
            .iter()
            .map(|column| column.name.as_str())
            .collect::<Vec<_>>(),
        ["ID", "Field1"]
    );
    assert!(rows.is_empty());

    let Response::Details {
        declared_rows,
        fields,
        ..
    } = request(
        &reader,
        Command::Describe {
            table: "Contacts".into(),
        },
    )
    .unwrap()
    else {
        panic!("expected the saved original native declarations");
    };
    assert_eq!(declared_rows, 0);
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].name, "ID");
    assert_eq!(
        (
            fields[0].number,
            fields[0].native_type,
            fields[0].storage_bytes,
            fields[0].fixed,
            fields[0].flags
        ),
        (0, 4, 4, true, 7)
    );
    assert_eq!(fields[1].name, "Field1");
    assert_eq!(
        (
            fields[1].number,
            fields[1].native_type,
            fields[1].storage_bytes,
            fields[1].fixed,
            fields[1].flags
        ),
        (1, 10, 510, false, 2)
    );
}

#[test]
fn replacing_the_document_invalidates_review_without_writing_or_creating_a_journal() {
    let (_directory, path, reader, revision) = opened();
    let capability = reviewed(&reader, revision, "Contacts");
    request(
        &reader,
        Command::Open {
            path: path.clone(),
            password: String::new(),
        },
    )
    .unwrap();
    let saver = SaveWorker::new(|| {});
    let rejected = saver
        .start_table_creation(capability, journal_path(&path))
        .unwrap_err();
    assert_eq!(rejected.kind, SaveStartFailure::StaleDocument);
    assert!(rejected.into_reviewed().is_some());
    assert_eq!(
        std::fs::read(&path).unwrap(),
        canonical_blank_creation::source()
    );
    assert!(!journal_path(&path).exists());
    assert!(!saver.is_busy());
}

#[test]
fn reopen_witness_does_not_accept_before_bytes_or_a_changed_after_image() {
    let (_directory, path, reader, revision) = opened();
    let capability = reviewed(&reader, revision, "Contacts");
    let witness = capability.reopen_witness();
    let other = Worker::new(|| {});
    assert!(matches!(
        request(
            &other,
            Command::OpenTableCreated {
                witness: witness.clone()
            }
        ),
        Err(Error::SavedTableSourceChanged)
    ));
    let saver = SaveWorker::new(|| {});
    let operation = saver
        .start_table_creation(capability, journal_path(&path))
        .unwrap();
    assert!(matches!(
        terminal(&saver).outcome,
        SaveOutcome::Saved { .. }
    ));
    assert!(saver.acknowledge(operation));
    let mut changed = std::fs::read(&path).unwrap();
    *changed.last_mut().unwrap() ^= 1;
    std::fs::write(&path, &changed).unwrap();
    assert!(matches!(
        request(&other, Command::OpenTableCreated { witness }),
        Err(Error::SavedTableSourceChanged)
    ));
    assert_eq!(std::fs::read(&path).unwrap(), changed);
}

#[test]
fn common_initial_fields_save_and_reopen_with_native_types_and_explicit_defaults() {
    use crate::FieldPropertyValue;

    let (_directory, path, reader, document_revision) = opened();
    let mut draft = TableDraft::initial();
    for kind in [
        DraftFieldType::Number,
        DraftFieldType::LargeInteger,
        DraftFieldType::YesNo,
        DraftFieldType::Currency,
        DraftFieldType::DateTime,
    ] {
        draft.add_field(kind).unwrap();
    }
    let Response::TableCreationReviewed { capability } = request(
        &reader,
        Command::ReviewTableCreation {
            document_revision,
            snapshot: draft.snapshot("TypedFields").unwrap(),
        },
    )
    .unwrap() else {
        panic!("expected a checked initial schema");
    };
    let witness = capability.reopen_witness();
    let saver = SaveWorker::new(|| {});
    let operation = saver
        .start_table_creation(capability, journal_path(&path))
        .unwrap();
    assert!(matches!(
        terminal(&saver).outcome,
        SaveOutcome::Saved { .. }
    ));
    assert!(saver.acknowledge(operation));
    request(&reader, Command::OpenTableCreated { witness }).unwrap();
    let Response::Details {
        fields,
        declared_rows,
        ..
    } = request(
        &reader,
        Command::Describe {
            table: "TypedFields".into(),
        },
    )
    .unwrap()
    else {
        panic!("expected reopened native field metadata");
    };
    assert_eq!(declared_rows, 0);
    assert_eq!(
        fields
            .iter()
            .map(|field| field.native_type)
            .collect::<Vec<_>>(),
        [4, 4, 19, 1, 5, 8]
    );
    let defaults = fields
        .iter()
        .map(|field| {
            &field
                .properties
                .iter()
                .find(|property| property.name == "DefaultValue")
                .unwrap()
                .value
        })
        .collect::<Vec<_>>();
    assert_eq!(
        defaults,
        [
            &FieldPropertyValue::Absent,
            &FieldPropertyValue::Text("0".into()),
            &FieldPropertyValue::Text("0".into()),
            &FieldPropertyValue::Text("False".into()),
            &FieldPropertyValue::Text("0.0000".into()),
            &FieldPropertyValue::Absent,
        ]
    );
}

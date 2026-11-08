//! The shared service selects append creation from its captured source profile.
use super::{opened_source, request, reviewed, terminal};
use crate::{
    Command, EditInput, Error, InsertInput, InsertTarget, Response, SaveOutcome, SaveSchemaChange,
    SaveStartFailure, SaveWorker, Worker, journal_path,
};
use tablecraft_testkit::fixtures::access::canonical_add_table;

fn assert_row(reader: &Worker, table: &str, expected: &str) {
    let Response::Data { rows, .. } = request(
        reader,
        Command::Page {
            table: table.into(),
            offset: 0,
        },
    )
    .unwrap() else {
        panic!("expected the saved table");
    };
    assert_eq!(rows.len(), 1, "{table}");
    assert_eq!(rows[0][0].text, "1", "{table}");
    assert_eq!(rows[0][1].text, expected, "{table}");
}

#[test]
fn repeated_reviewed_schema_saves_and_first_inserts_preserve_all_preceding_tables() {
    let source = canonical_add_table::source();
    let (_directory, path, reader, mut revision) = opened_source(&source.before);
    let saver = SaveWorker::new(|| {});
    let additions = [("Contacts", "Ada"), ("Third", "Grace"), ("Fourth", "Linus")];
    for (index, (table, value)) in additions.iter().enumerate() {
        let before = std::fs::read(&path).unwrap();
        assert_row(&reader, "Items", "17");
        for (old_table, old_value) in additions.iter().take(index) {
            assert_row(&reader, old_table, old_value);
        }
        let capability = reviewed(&reader, revision, table);
        let witness = capability.reopen_witness();
        assert_eq!(
            witness.expected_schema_change(),
            SaveSchemaChange::TableAdded { column_count: 2 }
        );
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert!(!journal_path(&path).exists());

        let operation = saver
            .start_table_creation(capability, journal_path(&path))
            .unwrap();
        let report = terminal(&saver);
        let SaveOutcome::Saved { receipt } = report.outcome else {
            panic!("expected a completed append-table save: {report:?}");
        };
        assert_eq!(
            receipt.schema_change,
            Some(witness.expected_schema_change())
        );
        assert_eq!((receipt.matched_rows, receipt.changed_rows), (None, None));
        assert!(saver.acknowledge(operation));
        assert_eq!(std::fs::read(&path).unwrap(), witness.expected_bytes());
        assert!(!journal_path(&path).exists());
        let Response::Opened {
            document_revision,
            tables,
            table_creation_available,
            ..
        } = request(&reader, Command::OpenTableCreated { witness }).unwrap()
        else {
            panic!("expected exact complete-image reopening");
        };
        assert_ne!(document_revision, revision);
        assert_eq!(
            tables.iter().filter(|table| !table.system).count(),
            index + 2
        );
        assert!(table_creation_available);
        assert_row(&reader, "Items", "17");
        let Response::InsertDraft(draft) = request(
            &reader,
            Command::BeginInsert {
                target: InsertTarget {
                    document_revision,
                    table: (*table).into(),
                },
            },
        )
        .unwrap() else {
            panic!("expected fresh insertion metadata for the added table");
        };
        let Response::InsertReviewed { capability, .. } = request(
            &reader,
            Command::ReviewInsert {
                draft_id: draft.id,
                inputs: vec![
                    InsertInput::Omitted,
                    InsertInput::Value(EditInput {
                        text: (*value).into(),
                        is_null: false,
                    }),
                ],
            },
        )
        .unwrap() else {
            panic!("expected ordinary reviewed insertion into the added table");
        };
        let operation = saver.start_insert(capability, journal_path(&path)).unwrap();
        assert!(
            matches!(terminal(&saver).outcome, SaveOutcome::Saved { receipt } if receipt.changed_rows == Some(1) && receipt.schema_change.is_none())
        );
        assert!(saver.acknowledge(operation));
        let Response::Opened {
            document_revision,
            table_creation_available,
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
            panic!("expected a fresh document after the separately committed record");
        };
        revision = document_revision;
        assert!(table_creation_available);
        assert_row(&reader, "Items", "17");
        for (saved_table, saved_value) in additions.iter().take(index + 1) {
            assert_row(&reader, saved_table, saved_value);
        }
        assert!(!journal_path(&path).exists());
    }
}

#[test]
fn added_schema_reviews_and_reopen_witnesses_refuse_stale_or_changed_sources() {
    let source = canonical_add_table::source();
    let (_directory, path, reader, revision) = opened_source(&source.before);
    let stale = reviewed(&reader, revision, "Contacts");
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
        .start_table_creation(stale, journal_path(&path))
        .unwrap_err();
    assert_eq!(rejected.kind, SaveStartFailure::StaleDocument);
    assert!(rejected.into_reviewed().is_some());
    assert!(!saver.is_busy());
    assert_eq!(std::fs::read(&path).unwrap(), source.before);
    assert!(!journal_path(&path).exists());

    let (_other_directory, other_path, current, revision) = opened_source(&source.before);
    let capability = reviewed(&current, revision, "Contacts");
    let witness = capability.reopen_witness();
    let observer = Worker::new(|| {});
    assert!(matches!(
        request(
            &observer,
            Command::OpenTableCreated {
                witness: witness.clone()
            }
        ),
        Err(Error::SavedTableSourceChanged)
    ));
    let operation = saver
        .start_table_creation(capability, journal_path(&other_path))
        .unwrap();
    assert!(matches!(
        terminal(&saver).outcome,
        SaveOutcome::Saved { .. }
    ));
    assert!(saver.acknowledge(operation));
    let mut changed = std::fs::read(&other_path).unwrap();
    // A byte in the old source is part of the complete expected AFTER too.
    changed[7 * 4096 + 3500] ^= 1;
    std::fs::write(&other_path, &changed).unwrap();
    assert!(matches!(
        request(&observer, Command::OpenTableCreated { witness }),
        Err(Error::SavedTableSourceChanged)
    ));
    assert_eq!(std::fs::read(&other_path).unwrap(), changed);
    assert!(!journal_path(&other_path).exists());
}

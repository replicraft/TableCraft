//! First-record entry follows actual first-schema saving and guarded reopening.
use super::{terminal, value};
use crate::tests::{open, request, stored::opened};
use crate::{
    Command, EditError, Error, InsertDraft, InsertInput, InsertTarget, Response, SaveKnownOutcome,
    SaveOutcome, SaveSchemaChange, SaveWorker, Worker, journal_path,
};
use std::path::PathBuf;
use tablecraft_builders::table::{DraftFieldType, TableDraft};
use tablecraft_testkit::fixtures::access::canonical_blank_creation;

#[derive(Clone, Copy)]
enum Companion {
    None,
    LongInteger,
    ShortText,
}

fn saved_initial(companion: Companion) -> (tempfile::TempDir, PathBuf, Worker, SaveWorker, u64) {
    let (directory, path, worker, revision) = opened(&canonical_blank_creation::source());
    let mut draft = TableDraft::initial();
    match companion {
        Companion::None => {}
        Companion::LongInteger => {
            draft.add_field(DraftFieldType::LongInteger).unwrap();
        }
        Companion::ShortText => {
            draft.add_field(DraftFieldType::ShortText).unwrap();
        }
    }
    let snapshot = draft.snapshot("Items").unwrap();
    let columns = snapshot.fields().len();
    let Response::FirstTableReviewed { capability } = request(
        &worker,
        Command::ReviewFirstTable {
            document_revision: revision,
            snapshot,
        },
    )
    .unwrap() else {
        panic!("expected actual initial-schema review");
    };
    let witness = capability.reopen_witness();
    let saves = SaveWorker::new(|| {});
    let operation = saves
        .start_first_table(capability, journal_path(&path))
        .unwrap();
    let report = terminal(&saves);
    let SaveOutcome::Saved { receipt } = report.outcome else {
        panic!("initial schema failed: {report:?}");
    };
    assert_eq!(report.id, operation);
    assert_eq!(receipt.known, SaveKnownOutcome::Committed);
    assert_eq!(
        receipt.retirement,
        Some(crate::SaveRetirementGuarantee::CapturedParentBarrier)
    );
    assert_eq!(
        receipt.cleanup,
        Some(crate::SaveCleanupDisposition::Removed)
    );
    assert_eq!(receipt.changed_rows, None);
    assert_eq!(
        receipt.schema_change,
        Some(SaveSchemaChange::FirstTableCreated {
            column_count: columns
        })
    );
    assert!(saves.acknowledge(operation));
    let Response::Opened {
        document_revision, ..
    } = request(&worker, Command::OpenFirstTableSaved { witness }).unwrap()
    else {
        panic!("expected guarded native reopening of the actual saved schema");
    };
    assert!(!journal_path(&path).exists());
    (directory, path, worker, saves, document_revision)
}

fn begin(worker: &Worker, revision: u64) -> InsertDraft {
    let Response::InsertDraft(draft) = request(
        worker,
        Command::BeginInsert {
            target: InsertTarget {
                document_revision: revision,
                table: "Items".into(),
            },
        },
    )
    .unwrap() else {
        panic!("expected actual native insertion metadata");
    };
    assert_eq!(draft.fields[0].name, "ID");
    assert!(draft.fields[0].auto_number);
    assert!(draft.fields[1..].iter().all(|field| !field.auto_number));
    draft
}

#[test]
fn first_row_uses_native_generated_id_in_all_three_saved_initial_schemas() {
    for companion in [
        Companion::None,
        Companion::LongInteger,
        Companion::ShortText,
    ] {
        let (_directory, path, worker, saves, revision) = saved_initial(companion);
        let before = std::fs::read(&path).unwrap();
        let draft = begin(&worker, revision);
        let mut inputs = vec![InsertInput::Omitted];
        let expected = match companion {
            Companion::None => None,
            Companion::LongInteger => {
                inputs.push(value("42", false));
                Some("42")
            }
            Companion::ShortText => {
                inputs.push(value("Ada", false));
                Some("Ada")
            }
        };
        let Response::InsertReviewed { review, capability } = request(
            &worker,
            Command::ReviewInsert {
                draft_id: draft.id,
                inputs,
            },
        )
        .unwrap() else {
            panic!("expected source-bound first-row review");
        };
        assert_eq!(review.inserted_rows, 1);
        assert!(matches!(review.inputs[0], InsertInput::Omitted));
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert!(!journal_path(&path).exists());

        let operation = saves.start_insert(capability, journal_path(&path)).unwrap();
        let report = terminal(&saves);
        let SaveOutcome::Saved { receipt } = report.outcome else {
            panic!("initial first row failed: {report:?}");
        };
        assert_eq!(report.id, operation);
        assert_eq!(receipt.known, SaveKnownOutcome::Committed);
        assert_eq!(
            receipt.retirement,
            Some(crate::SaveRetirementGuarantee::CapturedParentBarrier)
        );
        assert_eq!(
            receipt.cleanup,
            Some(crate::SaveCleanupDisposition::Removed)
        );
        assert_eq!(receipt.matched_rows, None);
        assert_eq!(receipt.changed_rows, Some(1));
        assert_eq!(receipt.schema_change, None);
        assert!(saves.acknowledge(operation));
        assert!(!journal_path(&path).exists());

        request(&worker, open(&path, "")).unwrap();
        let Response::Data { rows, columns, .. } = request(
            &worker,
            Command::Page {
                table: "Items".into(),
                offset: 0,
            },
        )
        .unwrap() else {
            panic!("expected actual persisted first record");
        };
        assert_eq!(rows.len(), 1);
        assert_eq!(columns.len(), draft.fields.len());
        assert_eq!(rows[0][0].text, "1");
        if let Some(expected) = expected {
            assert_eq!(rows[0][1].text, expected);
        }
    }
}

#[test]
fn supplied_value_and_explicit_null_cannot_replace_generated_id_omission() {
    let (_directory, path, worker, saves, revision) = saved_initial(Companion::ShortText);
    let original = std::fs::read(&path).unwrap();
    let draft = begin(&worker, revision);
    for id in [value("99", false), value("", true)] {
        let result = request(
            &worker,
            Command::ReviewInsert {
                draft_id: draft.id,
                inputs: vec![id, value("Ada", false)],
            },
        );
        assert!(matches!(result, Err(Error::Edit(EditError::InvalidInput))));
        assert_eq!(std::fs::read(&path).unwrap(), original);
        assert!(!journal_path(&path).exists());
        assert!(!saves.is_busy());
    }
}

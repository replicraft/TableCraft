//! First-record entry follows actual first-schema saving and guarded reopening.
use super::{terminal, value};
use crate::tests::{open, request, stored::opened};
use crate::{
    CellKind, Command, EditError, Error, InsertDraft, InsertInput, InsertTarget, Response,
    SaveKnownOutcome, SaveOutcome, SaveReceipt, SaveSchemaChange, SaveWorker, Worker, journal_path,
};
use std::path::PathBuf;
use tablecraft_builders::table::{DraftDefaultMode, DraftFieldType, TableDraft};
use tablecraft_testkit::fixtures::access::canonical_blank_creation;

#[derive(Clone, Copy)]
enum Companion {
    None,
    LongInteger,
    DefaultLongInteger,
    ShortText,
    NullableShortText,
    RequiredShortText,
}

fn assert_retirement(receipt: SaveReceipt) {
    let expected = if cfg!(windows) {
        (
            Some(crate::SaveRetirementGuarantee::NtfsWriteThroughRename),
            Some(crate::SaveCleanupDisposition::RetainedArtifact),
        )
    } else {
        (
            Some(crate::SaveRetirementGuarantee::CapturedParentBarrier),
            Some(crate::SaveCleanupDisposition::Removed),
        )
    };
    assert_eq!((receipt.retirement, receipt.cleanup), expected);
}

fn saved_initial(companion: Companion) -> (tempfile::TempDir, PathBuf, Worker, SaveWorker, u64) {
    let (directory, path, worker, revision) = opened(&canonical_blank_creation::source());
    let mut draft = TableDraft::initial();
    match companion {
        Companion::None => {}
        Companion::LongInteger | Companion::DefaultLongInteger => {
            draft.add_field(DraftFieldType::LongInteger).unwrap();
        }
        Companion::ShortText | Companion::NullableShortText | Companion::RequiredShortText => {
            let field = draft.add_field(DraftFieldType::ShortText).unwrap();
            if matches!(
                companion,
                Companion::NullableShortText | Companion::RequiredShortText
            ) {
                draft
                    .set_field_default(field, DraftDefaultMode::Absent)
                    .unwrap();
            }
            if matches!(companion, Companion::RequiredShortText) {
                draft.set_field_required(field, Some(true)).unwrap();
            }
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
    assert_retirement(receipt);
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
fn first_row_uses_native_generated_id_and_only_the_stored_companion_default() {
    for companion in [
        Companion::None,
        Companion::LongInteger,
        Companion::DefaultLongInteger,
        Companion::ShortText,
        Companion::NullableShortText,
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
            Companion::DefaultLongInteger => {
                inputs.push(InsertInput::Omitted);
                Some("0")
            }
            Companion::ShortText => {
                inputs.push(value("Ada", false));
                Some("Ada")
            }
            Companion::NullableShortText => {
                inputs.push(InsertInput::Omitted);
                None
            }
            Companion::RequiredShortText => unreachable!("tested by the refusal case"),
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
        if matches!(
            companion,
            Companion::DefaultLongInteger | Companion::NullableShortText
        ) {
            assert!(matches!(review.inputs[1], InsertInput::Omitted));
        }
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert!(!journal_path(&path).exists());

        let operation = saves.start_insert(capability, journal_path(&path)).unwrap();
        let report = terminal(&saves);
        let SaveOutcome::Saved { receipt } = report.outcome else {
            panic!("initial first row failed: {report:?}");
        };
        assert_eq!(report.id, operation);
        assert_eq!(receipt.known, SaveKnownOutcome::Committed);
        assert_retirement(receipt);
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
        if matches!(companion, Companion::NullableShortText) {
            assert!(matches!(
                rows[0][1].kind,
                CellKind::Scalar { is_null: true, .. }
            ));
        }
    }
}

#[test]
fn required_field_without_default_refuses_omission_then_accepts_explicit_value() {
    let (_directory, path, worker, saves, revision) = saved_initial(Companion::RequiredShortText);
    let before = std::fs::read(&path).unwrap();
    let draft = begin(&worker, revision);
    assert!(
        request(
            &worker,
            Command::ReviewInsert {
                draft_id: draft.id,
                inputs: vec![InsertInput::Omitted, InsertInput::Omitted],
            },
        )
        .is_err()
    );
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert!(!journal_path(&path).exists());
    assert!(!saves.is_busy());

    let Response::InsertReviewed { capability, .. } = request(
        &worker,
        Command::ReviewInsert {
            draft_id: draft.id,
            inputs: vec![InsertInput::Omitted, value("Ada", false)],
        },
    )
    .unwrap() else {
        panic!("expected corrected source-bound first-row review");
    };
    let operation = saves.start_insert(capability, journal_path(&path)).unwrap();
    let report = terminal(&saves);
    assert!(matches!(report.outcome, SaveOutcome::Saved { .. }));
    assert!(saves.acknowledge(operation));
    request(&worker, open(&path, "")).unwrap();
    let Response::Data { rows, .. } = request(
        &worker,
        Command::Page {
            table: "Items".into(),
            offset: 0,
        },
    )
    .unwrap() else {
        panic!("expected the corrected persisted record");
    };
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0][0].text, "1");
    assert_eq!(rows[0][1].text, "Ada");
    assert!(matches!(
        rows[0][1].kind,
        CellKind::Scalar { is_null: false, .. }
    ));
    assert!(!journal_path(&path).exists());
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

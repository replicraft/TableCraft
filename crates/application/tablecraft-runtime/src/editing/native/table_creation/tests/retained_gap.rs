//! A real native schema save must remain usable for subsequent reviewed row CRUD.
use super::{opened_source, request, terminal};
use crate::{
    CellKind, Command, DeleteTarget, EditInput, EditTarget, FieldPropertyValue, InsertInput,
    InsertTarget, Response, SaveOutcome, SaveSchemaChange, SaveWorker, Worker, journal_path,
};
use std::path::Path;
use tablecraft_builders::table::{DraftFieldType, TableDraft};
use tablecraft_testkit::fixtures::access::{
    canonical_retained_add_table::{deletes, rows, updates},
    retained_catalog_gap,
};

fn reopen(worker: &Worker, path: &Path) -> u64 {
    let Response::Opened {
        document_revision, ..
    } = request(
        worker,
        Command::Open {
            path: path.into(),
            password: String::new(),
        },
    )
    .unwrap()
    else {
        panic!("expected a fresh complete native document");
    };
    document_revision
}

fn assert_row(worker: &Worker, expected: Option<&str>) {
    let Response::Data { rows, .. } = request(
        worker,
        Command::Page {
            table: "Described".into(),
            offset: 0,
        },
    )
    .unwrap() else {
        panic!("expected the retained described table");
    };
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0][0].text, "1");
    match expected {
        Some(text) => assert_eq!(rows[0][1].text, text),
        None => assert!(matches!(
            rows[0][1].kind,
            CellKind::Scalar { is_null: true, .. }
        )),
    }
}

fn saved(saves: &SaveWorker, operation: u64, path: &Path, expected: &[u8]) {
    let report = terminal(saves);
    assert_eq!(report.id, operation);
    assert!(
        matches!(report.outcome, SaveOutcome::Saved { receipt } if receipt.changed_rows == Some(1) && receipt.schema_change.is_none()),
        "{report:?}"
    );
    assert!(saves.acknowledge(operation));
    assert_eq!(std::fs::read(path).unwrap(), expected);
    assert!(!journal_path(path).exists());
}

#[test]
fn native_creation_with_indirect_catalog_candidates_continues_through_row_crud() {
    let source = retained_catalog_gap::source();
    let definition = (source.len() / 4096) as u32;
    let (_directory, path, worker, revision) = opened_source(&source);
    let description = "D".repeat(255);
    let mut draft = TableDraft::initial();
    let score = draft
        .add_named_field(DraftFieldType::Number, "Score")
        .unwrap();
    draft
        .set_field_description(score, Some(&description))
        .unwrap();
    let Response::TableCreationReviewed { capability } = request(
        &worker,
        Command::ReviewTableCreation {
            document_revision: revision,
            snapshot: draft.snapshot("Described").unwrap(),
        },
    )
    .unwrap() else {
        panic!("expected a native property-placement review");
    };
    let witness = capability.reopen_witness();
    assert_eq!(
        witness.expected_schema_change(),
        SaveSchemaChange::TableAdded { column_count: 2 }
    );
    assert_eq!(std::fs::read(&path).unwrap(), source);
    assert!(!journal_path(&path).exists());
    let saves = SaveWorker::new(|| {});
    let operation = saves
        .start_table_creation(capability, journal_path(&path))
        .unwrap();
    let report = terminal(&saves);
    assert!(
        matches!(report.outcome, SaveOutcome::Saved { receipt } if receipt.schema_change == Some(SaveSchemaChange::TableAdded { column_count: 2 })),
        "{report:?}"
    );
    assert!(saves.acknowledge(operation));
    assert_eq!(std::fs::read(&path).unwrap(), witness.expected_bytes());
    let Response::Opened {
        document_revision, ..
    } = request(&worker, Command::OpenTableCreated { witness }).unwrap()
    else {
        panic!("expected guarded native-schema reopening");
    };
    let Response::Details {
        fields,
        declared_rows,
        ..
    } = request(
        &worker,
        Command::Describe {
            table: "Described".into(),
        },
    )
    .unwrap()
    else {
        panic!("expected exact persisted field properties");
    };
    assert_eq!(declared_rows, 0);
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[1].name, "Score");
    assert_eq!(
        fields[1]
            .properties
            .iter()
            .find(|property| property.name == "Description")
            .unwrap()
            .value,
        FieldPropertyValue::Text(description)
    );
    let created = std::fs::read(&path).unwrap();
    let expected_insert = rows::append(
        &created,
        definition,
        rows::Profile::DesktopNumber,
        rows::Input::Number(123),
    );
    let Response::InsertDraft(insertion) = request(
        &worker,
        Command::BeginInsert {
            target: InsertTarget {
                document_revision,
                table: "Described".into(),
            },
        },
    )
    .unwrap() else {
        panic!("expected a fresh insertion draft after native schema creation");
    };
    let Response::InsertReviewed { capability, .. } = request(
        &worker,
        Command::ReviewInsert {
            draft_id: insertion.id,
            inputs: vec![
                InsertInput::Omitted,
                InsertInput::Value(EditInput {
                    text: "123".into(),
                    is_null: false,
                }),
            ],
        },
    )
    .unwrap() else {
        panic!("expected a reviewed first row with its original presence");
    };
    assert_eq!(std::fs::read(&path).unwrap(), created);
    assert!(!journal_path(&path).exists());
    let operation = saves.start_insert(capability, journal_path(&path)).unwrap();
    saved(&saves, operation, &path, &expected_insert);
    let mut revision = reopen(&worker, &path);
    assert_row(&worker, Some("123"));
    let mut expected = expected_insert;
    for value in [Some(124), None, Some(-42)] {
        let before = expected;
        expected = updates::number(&before, definition, 0, value);
        let Response::EditDraft(edit) = request(
            &worker,
            Command::BeginEdit {
                target: EditTarget {
                    document_revision: revision,
                    table: "Described".into(),
                    column: "Score".into(),
                    parent_ordinal: 0,
                },
            },
        )
        .unwrap() else {
            panic!("expected a source-bound existing-cell draft");
        };
        let Response::EditReviewed { capability, .. } = request(
            &worker,
            Command::ReviewEdit {
                draft_id: edit.id,
                input: EditInput {
                    text: value.map_or_else(String::new, |value| value.to_string()),
                    is_null: value.is_none(),
                },
            },
        )
        .unwrap() else {
            panic!("expected reviewed numeric replacement");
        };
        assert_eq!(std::fs::read(&path).unwrap(), before);
        let operation = saves.start(capability, journal_path(&path)).unwrap();
        saved(&saves, operation, &path, &expected);
        revision = reopen(&worker, &path);
        let displayed = value.map(|value| value.to_string());
        assert_row(&worker, displayed.as_deref());
    }
    let expected_delete = deletes::after(&expected, definition, &[0]);
    let Response::DeleteReviewed { capability, .. } = request(
        &worker,
        Command::ReviewDelete {
            target: DeleteTarget {
                document_revision: revision,
                table: "Described".into(),
                parent_ordinal: 0,
            },
        },
    )
    .unwrap() else {
        panic!("expected deletion of the retained first row");
    };
    let operation = saves.start_delete(capability, journal_path(&path)).unwrap();
    saved(&saves, operation, &path, &expected_delete);
    reopen(&worker, &path);
    let Response::Data { rows, .. } = request(
        &worker,
        Command::Page {
            table: "Described".into(),
            offset: 0,
        },
    )
    .unwrap() else {
        panic!("expected an empty retained table");
    };
    assert!(rows.is_empty());
}

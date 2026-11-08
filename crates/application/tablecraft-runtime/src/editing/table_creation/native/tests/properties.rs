//! Real schema review, durable save and fresh native property observations.
use super::{opened, request, terminal};
use crate::{
    Command, Field, FieldPropertyValue, InsertInput, InsertTarget, Response, SaveOutcome,
    SaveWorker, Worker, journal_path,
};
use std::path::PathBuf;
use tablecraft_builders::table::{DraftDefaultMode, DraftFieldType, TableDraft};
use tablecraft_testkit::fixtures::access::canonical_blank_creation;

fn persist(draft: TableDraft) -> (tempfile::TempDir, PathBuf, Worker, Vec<Field>) {
    let (directory, path, reader, document_revision) = opened();
    let Response::TableCreationReviewed { capability } = request(
        &reader,
        Command::ReviewTableCreation {
            document_revision,
            snapshot: draft.snapshot("Properties").unwrap(),
        },
    )
    .unwrap() else {
        panic!("expected an actual property-bearing schema review");
    };
    assert_eq!(
        std::fs::read(&path).unwrap(),
        canonical_blank_creation::source()
    );
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
            table: "Properties".into(),
        },
    )
    .unwrap()
    else {
        panic!("expected native metadata after guarded reopening");
    };
    assert_eq!(declared_rows, 0);
    (directory, path, reader, fields)
}

fn property<'a>(field: &'a Field, name: &str) -> &'a FieldPropertyValue {
    &field
        .properties
        .iter()
        .find(|entry| entry.name == name)
        .unwrap()
        .value
}

#[test]
fn descriptions_reopen_as_exact_inert_text_with_empty_and_absent_distinct() {
    let mut draft = TableDraft::initial();
    let id = draft.fields()[0].id();
    let boundary = format!("{}😀", "é".repeat(253));
    draft.set_field_description(id, Some(&boundary)).unwrap();
    let named = draft.add_field(DraftFieldType::ShortText).unwrap();
    draft.rename_field(named, "Label").unwrap();
    let exact = "  Résumé 日本語 Now()  ";
    draft.set_field_description(named, Some(exact)).unwrap();
    let empty = draft.add_field(DraftFieldType::Number).unwrap();
    draft.set_field_description(empty, Some("")).unwrap();
    draft.add_field(DraftFieldType::YesNo).unwrap();

    let (_directory, path, reader, fields) = persist(draft);
    let expected = [
        FieldPropertyValue::Text(boundary),
        FieldPropertyValue::Text(exact.into()),
        FieldPropertyValue::Text(String::new()),
        FieldPropertyValue::Absent,
    ];
    assert_eq!(fields.len(), expected.len());
    for (field, expected) in fields.iter().zip(&expected) {
        assert_eq!(property(field, "Description"), expected);
    }
    drop(reader);

    // Read through a fresh runtime, so the assertion cannot reuse draft metadata
    // or a property cache belonging to the save/reopen session.
    let fresh = Worker::new(|| {});
    let Response::Opened {
        document_revision: mut revision,
        ..
    } = request(
        &fresh,
        Command::Open {
            path: path.clone(),
            password: String::new(),
        },
    )
    .unwrap()
    else {
        panic!("expected a fresh native runtime");
    };
    let Response::Details { fields, .. } = request(
        &fresh,
        Command::Describe {
            table: "Properties".into(),
        },
    )
    .unwrap() else {
        panic!("expected persisted native field descriptions");
    };
    assert_eq!(fields.len(), expected.len());
    for (field, expected) in fields.iter().zip(&expected) {
        assert_eq!(property(field, "Description"), expected);
    }

    let unchanged = std::fs::read(&path).unwrap();
    let Response::InsertDraft(insertion) = request(
        &fresh,
        Command::BeginInsert {
            target: InsertTarget {
                document_revision: revision,
                table: "Properties".into(),
            },
        },
    )
    .unwrap() else {
        panic!("expected described-table insertion metadata");
    };
    assert!(matches!(
        request(
            &fresh,
            Command::ReviewInsert {
                draft_id: insertion.id,
                inputs: (0..4).map(|_| InsertInput::Omitted).collect(),
            }
        )
        .unwrap(),
        Response::InsertReviewed { .. }
    ));
    assert_eq!(std::fs::read(&path).unwrap(), unchanged);
    assert!(!journal_path(&path).exists());

    // Both append source partitions must retain the original descriptions and
    // carry the new exact declaration through their own reviewed journal route.
    let saver = SaveWorker::new(|| {});
    for table in ["Second", "Third"] {
        let mut draft = TableDraft::initial();
        let field = draft.add_field(DraftFieldType::ShortText).unwrap();
        draft.set_field_description(field, Some(exact)).unwrap();
        let Response::TableCreationReviewed { capability } = request(
            &fresh,
            Command::ReviewTableCreation {
                document_revision: revision,
                snapshot: draft.snapshot(table).unwrap(),
            },
        )
        .unwrap() else {
            panic!("expected a description-bearing append review");
        };
        let witness = capability.reopen_witness();
        let operation = saver
            .start_table_creation(capability, journal_path(&path))
            .unwrap();
        let outcome = terminal(&saver).outcome;
        assert!(matches!(outcome, SaveOutcome::Saved { .. }), "{outcome:?}");
        assert!(saver.acknowledge(operation));
        let Response::Opened {
            document_revision, ..
        } = request(&fresh, Command::OpenTableCreated { witness }).unwrap()
        else {
            panic!("expected complete described-table reopening");
        };
        revision = document_revision;
        for name in ["Properties", table] {
            let Response::Details { fields, .. } =
                request(&fresh, Command::Describe { table: name.into() }).unwrap()
            else {
                panic!("expected stored descriptions after append");
            };
            if name == "Properties" {
                assert_eq!(fields.len(), expected.len());
                for (field, expected) in fields.iter().zip(&expected) {
                    assert_eq!(property(field, "Description"), expected);
                }
            } else {
                assert_eq!(
                    property(&fields[0], "Description"),
                    &FieldPropertyValue::Absent
                );
                assert_eq!(
                    property(&fields[1], "Description"),
                    &FieldPropertyValue::Text(exact.into())
                );
            }
        }
    }
}

#[test]
fn exact_capacity_policies_and_default_modes_survive_review_save_and_native_reopen() {
    let mut draft = TableDraft::initial();
    let text = draft.add_field(DraftFieldType::ShortText).unwrap();
    draft.rename_field(text, "ConstantText").unwrap();
    draft.set_field_text_units(text, 8).unwrap();
    draft.set_field_required(text, Some(true)).unwrap();
    draft
        .set_field_allow_zero_length(text, Some(false))
        .unwrap();
    draft
        .set_field_default(text, DraftDefaultMode::Literal("Now()".into()))
        .unwrap();
    let absent = draft.add_field(DraftFieldType::Number).unwrap();
    draft
        .set_field_default(absent, DraftDefaultMode::Absent)
        .unwrap();
    let null = draft.add_field(DraftFieldType::Number).unwrap();
    draft.set_field_required(null, Some(false)).unwrap();
    draft
        .set_field_default(null, DraftDefaultMode::Null)
        .unwrap();
    let flag = draft.add_field(DraftFieldType::YesNo).unwrap();
    draft
        .set_field_default(flag, DraftDefaultMode::Literal("true".into()))
        .unwrap();
    let number = draft.add_field(DraftFieldType::Number).unwrap();
    draft
        .set_field_default(number, DraftDefaultMode::Literal("0042".into()))
        .unwrap();

    let (_directory, _path, _reader, fields) = persist(draft);
    assert_eq!(fields[1].name, "ConstantText");
    assert_eq!(fields[1].storage_bytes, 16);
    assert_eq!(
        property(&fields[1], "Required"),
        &FieldPropertyValue::Boolean(true)
    );
    assert_eq!(
        property(&fields[1], "AllowZeroLength"),
        &FieldPropertyValue::Boolean(false)
    );
    assert_eq!(
        property(&fields[1], "DefaultValue"),
        &FieldPropertyValue::Text("\"Now()\"".into())
    );
    assert_eq!(
        property(&fields[2], "DefaultValue"),
        &FieldPropertyValue::Absent
    );
    assert_eq!(
        property(&fields[3], "Required"),
        &FieldPropertyValue::Boolean(false)
    );
    assert_eq!(
        property(&fields[3], "DefaultValue"),
        &FieldPropertyValue::Text("Null".into())
    );
    assert_eq!(
        property(&fields[4], "DefaultValue"),
        &FieldPropertyValue::Text("True".into())
    );
    assert_eq!(
        property(&fields[5], "DefaultValue"),
        &FieldPropertyValue::Text("42".into())
    );
}

#[test]
fn invalid_literal_or_native_expression_capacity_refuses_without_a_journal_or_write() {
    let cases = [
        (
            DraftFieldType::Number,
            DraftDefaultMode::Literal("2147483648".into()),
        ),
        (
            DraftFieldType::DateTime,
            DraftDefaultMode::Literal("Now()".into()),
        ),
        (DraftFieldType::YesNo, DraftDefaultMode::Null),
        // Native default expressions include their quotes and are limited to 255 units.
        (
            DraftFieldType::ShortText,
            DraftDefaultMode::Literal("x".repeat(254)),
        ),
    ];
    for (kind, mode) in cases {
        let (_directory, path, reader, document_revision) = opened();
        let mut draft = TableDraft::initial();
        let field = draft.add_field(kind).unwrap();
        draft.set_field_default(field, mode).unwrap();
        assert!(
            request(
                &reader,
                Command::ReviewTableCreation {
                    document_revision,
                    snapshot: draft.snapshot("Properties").unwrap(),
                }
            )
            .is_err()
        );
        assert!(!journal_path(&path).exists());
        assert_eq!(
            std::fs::read(&path).unwrap(),
            canonical_blank_creation::source()
        );

        draft
            .set_field_default(field, DraftDefaultMode::TypeDefault)
            .unwrap();
        assert!(matches!(
            request(
                &reader,
                Command::ReviewTableCreation {
                    document_revision,
                    snapshot: draft.snapshot("Properties").unwrap(),
                }
            )
            .unwrap(),
            Response::TableCreationReviewed { .. }
        ));
    }
}

//! Manual declarations use the same consuming schema and row save services.
use super::{opened, request, terminal};
use crate::{
    Command, EditInput, InsertInput, InsertTarget, Response, SaveOutcome, SaveWorker, Worker,
    journal_path,
};
use std::io::Cursor;
use tablecraft_ace::read_table;
use tablecraft_builders::table::{DraftFieldType, TableDraft};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Currency, Value},
};
use tablecraft_model::CatalogScope;

#[test]
fn manual_schemas_save_real_optional_keys_and_accept_the_first_explicit_record() {
    for (keyed, kind, native_type, text, value) in [
        (false, DraftFieldType::Number, 4, "7", Value::Int32(7)),
        (true, DraftFieldType::Number, 4, "7", Value::Int32(7)),
        (true, DraftFieldType::LargeInteger, 19, "7", Value::Int64(7)),
        (
            true,
            DraftFieldType::Currency,
            5,
            "7.5",
            Value::Currency(Currency::from_scaled(75_000)),
        ),
    ] {
        let (_directory, path, reader, revision) = opened();
        let mut draft = TableDraft::design();
        let name = draft.add_field(DraftFieldType::ShortText).unwrap();
        draft.rename_field(name, "Name").unwrap();
        draft
            .set_field_description(name, Some("  manual 日本語  "))
            .unwrap();
        let key = draft.add_field(kind).unwrap();
        draft.rename_field(key, "ActualKey").unwrap();
        if keyed {
            draft.set_primary_key(Some(key)).unwrap();
        }
        let original = std::fs::read(&path).unwrap();
        let Response::TableCreationReviewed { capability } = request(
            &reader,
            Command::ReviewTableCreation {
                document_revision: revision,
                snapshot: draft.snapshot("Manual").unwrap(),
            },
        )
        .unwrap() else {
            panic!("expected a source-bound manual schema review");
        };
        let witness = capability.reopen_witness();
        assert_eq!(std::fs::read(&path).unwrap(), original);
        assert!(!journal_path(&path).exists());
        let saver = SaveWorker::new(|| {});
        let operation = saver
            .start_table_creation(capability, journal_path(&path))
            .unwrap();
        let outcome = terminal(&saver).outcome;
        assert!(matches!(outcome, SaveOutcome::Saved { .. }), "{outcome:?}");
        assert!(saver.acknowledge(operation));
        assert_eq!(std::fs::read(&path).unwrap(), witness.expected_bytes());
        let Response::Opened {
            document_revision,
            table_design_available,
            ..
        } = request(&reader, Command::OpenTableCreated { witness }).unwrap()
        else {
            panic!("expected guarded manual-schema reopening");
        };
        assert!(table_design_available);
        let Response::Details { fields, .. } = request(
            &reader,
            Command::Describe {
                table: "Manual".into(),
            },
        )
        .unwrap() else {
            panic!("expected native manual field declarations");
        };
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].name, "Name");
        assert_eq!(fields[1].name, "ActualKey");
        assert_eq!(fields[1].native_type, native_type);
        assert!(fields.iter().all(|field| field.flags & 4 == 0));

        let Response::InsertDraft(insertion) = request(
            &reader,
            Command::BeginInsert {
                target: InsertTarget {
                    document_revision,
                    table: "Manual".into(),
                },
            },
        )
        .unwrap() else {
            panic!("expected ordinary insertion metadata for the manual table");
        };
        assert!(insertion.fields.iter().all(|field| !field.auto_number));
        let Response::InsertReviewed { capability, .. } = request(
            &reader,
            Command::ReviewInsert {
                draft_id: insertion.id,
                inputs: ["  Ada  ", text]
                    .into_iter()
                    .map(|text| {
                        InsertInput::Value(EditInput {
                            text: text.into(),
                            is_null: false,
                        })
                    })
                    .collect(),
            },
        )
        .unwrap() else {
            panic!("expected a separately reviewed explicit first record");
        };
        let operation = saver.start_insert(capability, journal_path(&path)).unwrap();
        let outcome = terminal(&saver).outcome;
        assert!(matches!(outcome, SaveOutcome::Saved { .. }), "{outcome:?}");
        assert!(saver.acknowledge(operation));
        drop(reader);
        let fresh = Worker::new(|| {});
        request(
            &fresh,
            Command::Open {
                path: path.clone(),
                password: String::new(),
            },
        )
        .unwrap();
        let Response::Data { rows, .. } = request(
            &fresh,
            Command::Page {
                table: "Manual".into(),
                offset: 0,
            },
        )
        .unwrap() else {
            panic!("expected the persisted record through a fresh runtime");
        };
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0][0].text, "  Ada  ");
        let bytes = std::fs::read(&path).unwrap();
        let native = read_table(
            &mut Cursor::new(bytes.as_slice()),
            CatalogScope::new(501),
            "Manual",
            &mut ResourceBudget::new(64 * 1024 * 1024, 100_000, 100_000_000),
            &|| false,
        )
        .unwrap();
        assert_eq!(native.rows(), &[vec![Value::Text("  Ada  ".into()), value]]);
        assert!(!journal_path(&path).exists());
    }
}

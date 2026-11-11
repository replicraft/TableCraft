//! Exact GUID and Decimal authoring precedes retained existing-cell updates.
use super::{opened_source, request, terminal};
use crate::{
    Command, EditInput, EditTarget, InsertInput, InsertTarget, Response, SaveOutcome,
    SaveSchemaChange, SaveWorker, Worker, journal_path,
};
use std::path::Path;
use tablecraft_builders::table::{DraftFieldType, DraftNumberSize, TableDraft};
use tablecraft_schema::DecimalConstraints;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    exact_values as raw, fields,
};

const TOKEN: [u8; 16] = [
    0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff,
];
const REPLACEMENT_TOKEN: [u8; 16] = [
    0xfe, 0xdc, 0xba, 0x98, 0x76, 0x54, 0x32, 0x10, 0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef,
];
const AMOUNT: i128 = 90_071_992_547_409_931_234;
const REPLACEMENT_AMOUNT: i128 = -90_071_992_547_409_934_321;

fn assert_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len(), "complete image length");
    assert_eq!(
        actual
            .iter()
            .zip(expected)
            .position(|(actual, expected)| actual != expected),
        None,
        "first differing image byte",
    );
}

fn draft() -> TableDraft {
    let mut draft = TableDraft::initial();
    draft
        .add_named_field(DraftFieldType::Number, "Score")
        .unwrap();
    let token = draft
        .add_named_field(DraftFieldType::Number, "Token")
        .unwrap();
    draft
        .set_field_number_size(token, DraftNumberSize::ReplicationId)
        .unwrap();
    let amount = draft
        .add_named_field(DraftFieldType::Number, "Amount")
        .unwrap();
    draft
        .set_field_number_size(amount, DraftNumberSize::Decimal)
        .unwrap();
    draft
        .set_field_decimal_constraints(amount, DecimalConstraints::new(28, 4).unwrap())
        .unwrap();
    draft
}

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
        panic!("expected fresh complete ExactValues source");
    };
    document_revision
}

fn save_row(saves: &SaveWorker, operation: u64, path: &Path, expected: &[u8]) {
    let report = terminal(saves);
    assert_eq!(report.id, operation);
    assert!(
        matches!(report.outcome, SaveOutcome::Saved { receipt } if receipt.changed_rows == Some(1) && receipt.schema_change.is_none()),
        "{report:?}"
    );
    assert!(saves.acknowledge(operation));
    assert_image(&std::fs::read(path).unwrap(), expected);
    assert!(!journal_path(path).exists());
}

fn review(worker: &Worker, revision: u64, column: &str, text: Option<&str>) -> crate::ReviewedEdit {
    let Response::EditDraft(edit) = request(
        worker,
        Command::BeginEdit {
            target: EditTarget {
                document_revision: revision,
                table: "ExactValues".into(),
                column: column.into(),
                parent_ordinal: 0,
            },
        },
    )
    .unwrap_or_else(|error| panic!("{column} begin: {error:?}")) else {
        panic!("{column}: expected a native existing-cell draft");
    };
    let Response::EditReviewed { capability, .. } = request(
        worker,
        Command::ReviewEdit {
            draft_id: edit.id,
            input: EditInput {
                text: text.unwrap_or("").into(),
                is_null: text.is_none(),
            },
        },
    )
    .unwrap_or_else(|error| panic!("{column} review: {error:?}")) else {
        panic!("{column}: expected exact typed replacement review");
    };
    capability
}

#[test]
fn retained_exact_values_create_insert_and_guid_decimal_edits() {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let definition = (seed.len() / 4096) as u32;
    let created = raw::append_table(&seed, "ExactValues");
    let (_directory, path, worker, revision) = opened_source(&seed);
    let Response::TableCreationReviewed { capability } = request(
        &worker,
        Command::ReviewTableCreation {
            document_revision: revision,
            snapshot: draft().snapshot("ExactValues").unwrap(),
        },
    )
    .unwrap() else {
        panic!("expected ExactValues schema review");
    };
    let witness = capability.reopen_witness();
    assert_image(witness.expected_bytes(), &created);
    assert_image(&std::fs::read(&path).unwrap(), &seed);
    let saves = SaveWorker::new(|| {});
    let operation = saves
        .start_table_creation(capability, journal_path(&path))
        .unwrap();
    let report = terminal(&saves);
    assert_eq!(report.id, operation);
    assert!(
        matches!(report.outcome, SaveOutcome::Saved { receipt }
        if receipt.schema_change == Some(SaveSchemaChange::TableAdded { column_count: 4 })),
        "{report:?}"
    );
    assert!(saves.acknowledge(operation));
    assert_image(&std::fs::read(&path).unwrap(), &created);
    let Response::Opened {
        document_revision, ..
    } = request(&worker, Command::OpenTableCreated { witness }).unwrap()
    else {
        panic!("expected exact schema reopening");
    };
    let Response::InsertDraft(insertion) = request(
        &worker,
        Command::BeginInsert {
            target: InsertTarget {
                document_revision,
                table: "ExactValues".into(),
            },
        },
    )
    .unwrap() else {
        panic!("expected ExactValues insertion metadata");
    };
    let inputs = vec![
        InsertInput::Omitted,
        InsertInput::Omitted,
        InsertInput::Value(EditInput {
            text: "00112233-4455-6677-8899-aabbccddeeff".into(),
            is_null: false,
        }),
        InsertInput::Value(EditInput {
            text: "9007199254740993.1234".into(),
            is_null: false,
        }),
    ];
    let Response::InsertReviewed { capability, .. } = request(
        &worker,
        Command::ReviewInsert {
            draft_id: insertion.id,
            inputs,
        },
    )
    .unwrap() else {
        panic!("expected exact GUID/Decimal insertion review");
    };
    let mut expected = raw::append(&created, definition, Some(TOKEN), Some(AMOUNT));
    let operation = saves.start_insert(capability, journal_path(&path)).unwrap();
    save_row(&saves, operation, &path, &expected);
    let mut revision = reopen(&worker, &path);

    let control = review(&worker, revision, "Score", Some("124"));
    expected = raw::update_score(&expected, definition, 0, Some(124));
    let operation = saves.start(control, journal_path(&path)).unwrap();
    save_row(&saves, operation, &path, &expected);
    revision = reopen(&worker, &path);

    for (text, value) in [
        (
            Some("fedcba98-7654-3210-0123-456789abcdef"),
            Some(REPLACEMENT_TOKEN),
        ),
        (None, None),
        (Some("00112233-4455-6677-8899-aabbccddeeff"), Some(TOKEN)),
    ] {
        let before = expected;
        let capability = review(&worker, revision, "Token", text);
        expected = raw::update_guid(&before, definition, 0, value);
        assert_image(&std::fs::read(&path).unwrap(), &before);
        let operation = saves.start(capability, journal_path(&path)).unwrap();
        save_row(&saves, operation, &path, &expected);
        revision = reopen(&worker, &path);
    }
    for (text, value) in [
        (Some("-9007199254740993.4321"), Some(REPLACEMENT_AMOUNT)),
        (None, None),
        (Some("9007199254740993.1234"), Some(AMOUNT)),
    ] {
        let before = expected;
        let capability = review(&worker, revision, "Amount", text);
        expected = raw::update_decimal(&before, definition, 0, value);
        assert_image(&std::fs::read(&path).unwrap(), &before);
        let operation = saves.start(capability, journal_path(&path)).unwrap();
        save_row(&saves, operation, &path, &expected);
        revision = reopen(&worker, &path);
    }
    let Response::Data { columns, rows, .. } = request(
        &worker,
        Command::Page {
            table: "ExactValues".into(),
            offset: 0,
        },
    )
    .unwrap() else {
        panic!("expected final exact typed row");
    };
    assert_eq!(columns.len(), 4);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0][0].text, "1");
    assert_eq!(rows[0][1].text, "124");
}

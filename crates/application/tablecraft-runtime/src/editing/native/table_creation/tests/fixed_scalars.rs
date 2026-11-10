//! Real authoring and insertion precede retained fixed-scalar cell editing.
use super::{opened_source, request, terminal};
use crate::{
    Command, EditInput, EditTarget, InsertInput, InsertTarget, Response, SaveOutcome,
    SaveSchemaChange, SaveWorker, Worker, journal_path,
};
use std::path::Path;
use tablecraft_builders::table::{DraftFieldType, DraftNumberSize, TableDraft};
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    fields,
    fixed_scalars::{self as raw, Column, Scalar},
};

const INITIAL: [(Column, &str, Scalar); 6] = [
    (Column::ByteValue, "17", Scalar::Byte(17)),
    (Column::ShortValue, "-1234", Scalar::Int16(-1234)),
    (
        Column::LargeValue,
        "9007199254740993",
        Scalar::Int64(9_007_199_254_740_993),
    ),
    (Column::SingleValue, "1.25", Scalar::SingleBits(0x3fa0_0000)),
    (
        Column::DoubleValue,
        "1.23456789012345",
        Scalar::DoubleBits(0x3ff3_c0ca_428c_59dd),
    ),
    (
        Column::DateValue,
        "1900-01-01T12:00:00",
        Scalar::DateTimeBits(0x4004_0000_0000_0000),
    ),
];
const REPLACEMENTS: [(&str, Scalar); 6] = [
    ("255", Scalar::Byte(255)),
    ("-32768", Scalar::Int16(i16::MIN)),
    ("-9007199254740993", Scalar::Int64(-9_007_199_254_740_993)),
    ("-2.5", Scalar::SingleBits(0xc020_0000)),
    ("-2.75", Scalar::DoubleBits(0xc006_0000_0000_0000)),
    (
        "1900-01-02T12:00:00",
        Scalar::DateTimeBits(0x400c_0000_0000_0000),
    ),
];

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
    for (name, size) in [
        ("ByteValue", DraftNumberSize::Byte),
        ("ShortValue", DraftNumberSize::Integer),
    ] {
        let field = draft.add_named_field(DraftFieldType::Number, name).unwrap();
        draft.set_field_number_size(field, size).unwrap();
    }
    draft
        .add_named_field(DraftFieldType::LargeInteger, "LargeValue")
        .unwrap();
    for (name, size) in [
        ("SingleValue", DraftNumberSize::Single),
        ("DoubleValue", DraftNumberSize::Double),
    ] {
        let field = draft.add_named_field(DraftFieldType::Number, name).unwrap();
        draft.set_field_number_size(field, size).unwrap();
    }
    draft
        .add_named_field(DraftFieldType::DateTime, "DateValue")
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
        panic!("expected fresh complete Measurements source");
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
                table: "Measurements".into(),
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
fn retained_measurements_create_insert_and_exact_fixed_scalar_edits() {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let definition = (seed.len() / 4096) as u32;
    let created = raw::append_table(&seed, "Measurements");
    let (_directory, path, worker, revision) = opened_source(&seed);
    let Response::TableCreationReviewed { capability } = request(
        &worker,
        Command::ReviewTableCreation {
            document_revision: revision,
            snapshot: draft().snapshot("Measurements").unwrap(),
        },
    )
    .unwrap() else {
        panic!("expected actual Measurements schema review");
    };
    let witness = capability.reopen_witness();
    assert_image(witness.expected_bytes(), &created);
    assert_image(&std::fs::read(&path).unwrap(), &seed);
    let saves = SaveWorker::new(|| {});
    let operation = saves
        .start_table_creation(capability, journal_path(&path))
        .unwrap();
    let report = terminal(&saves);
    assert!(
        matches!(report.outcome, SaveOutcome::Saved { receipt } if receipt.schema_change == Some(SaveSchemaChange::TableAdded { column_count: 8 })),
        "{report:?}"
    );
    assert!(saves.acknowledge(operation));
    assert_image(&std::fs::read(&path).unwrap(), &created);
    let Response::Opened {
        document_revision, ..
    } = request(&worker, Command::OpenTableCreated { witness }).unwrap()
    else {
        panic!("expected exact Measurements schema reopening");
    };
    let Response::InsertDraft(insertion) = request(
        &worker,
        Command::BeginInsert {
            target: InsertTarget {
                document_revision,
                table: "Measurements".into(),
            },
        },
    )
    .unwrap() else {
        panic!("expected Measurements insertion metadata");
    };
    let mut inputs = vec![InsertInput::Omitted, InsertInput::Omitted];
    inputs.extend(INITIAL.iter().map(|(_, text, _)| {
        InsertInput::Value(EditInput {
            text: (*text).into(),
            is_null: false,
        })
    }));
    let Response::InsertReviewed { capability, .. } = request(
        &worker,
        Command::ReviewInsert {
            draft_id: insertion.id,
            inputs,
        },
    )
    .unwrap() else {
        panic!("expected exact six-scalar insertion review");
    };
    let mut expected =
        raw::append_values(&created, definition, &INITIAL.map(|(_, _, value)| value));
    let operation = saves.start_insert(capability, journal_path(&path)).unwrap();
    save_row(&saves, operation, &path, &expected);
    let mut revision = reopen(&worker, &path);

    // This independent LONG control proves the existing retained edit service
    // works on the same table before a newly requested scalar action is tried.
    let control = review(&worker, revision, "Score", Some("124"));
    expected = raw::update_score(&expected, definition, 0, Some(124));
    let operation = saves.start(control, journal_path(&path)).unwrap();
    save_row(&saves, operation, &path, &expected);
    revision = reopen(&worker, &path);

    for ((column, initial_text, initial), (replacement_text, replacement)) in
        INITIAL.into_iter().zip(REPLACEMENTS)
    {
        let name = column.name();
        for (text, value) in [
            (Some(replacement_text), Some(replacement)),
            (None, None),
            (Some(initial_text), Some(initial)),
        ] {
            let before = expected;
            let capability = review(&worker, revision, name, text);
            expected = raw::update(&before, definition, 0, column, value);
            assert_image(&std::fs::read(&path).unwrap(), &before);
            let operation = saves.start(capability, journal_path(&path)).unwrap();
            save_row(&saves, operation, &path, &expected);
            revision = reopen(&worker, &path);
        }
    }
    let Response::Data { columns, rows, .. } = request(
        &worker,
        Command::Page {
            table: "Measurements".into(),
            offset: 0,
        },
    )
    .unwrap() else {
        panic!("expected the final typed scalar row");
    };
    assert_eq!(columns.len(), 8);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0][0].text, "1");
    assert_eq!(rows[0][1].text, "124");
}

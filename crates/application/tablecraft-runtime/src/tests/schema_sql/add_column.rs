//! SQL absence stays absent through the real consuming field-addition worker.
use super::{inspect, review};
use crate::tests::{request, stored::opened, terminal};
use crate::{
    Command, Error, FieldAdditionOrigin, InsertInput, InsertTarget, Response, SaveKnownOutcome,
    SaveOutcome, SaveSchemaChange, SaveWorker,
};
use std::{fs, io::Cursor};
use tablecraft_ace::{inspect_table_properties, read_table};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_core::values::{Currency, Value};
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    declared_fields as raw, field_changes::inserting,
};

const SQL: &str = "ALTER TABLE Items ADD COLUMN Amount MONEY";

fn same(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    assert_eq!(actual.iter().zip(expected).position(|(a, b)| a != b), None);
}

#[test]
fn sql_declared_currency_add_saves_without_gallery_policies_and_reopens_historical_null() {
    let mut source = raw::source();
    source.bytes = inserting::keyless_score(&source.bytes, source.definition, None);
    let expected = raw::after(&source.bytes, source.definition, "Amount");
    let (_directory, path, worker, revision) = opened(&source.bytes);
    let target = inspect(&worker, revision, SQL);
    let Response::FieldAdditionReviewed {
        review: facts,
        capability,
    } = review(&worker, &target).unwrap()
    else {
        panic!("expected the actual declared field-addition owner");
    };
    assert_eq!(
        facts.origin,
        FieldAdditionOrigin::SqlUnadorned(
            tablecraft_builders::table::TypedFieldAdditionDeclaration::fixed(ScalarType::Currency)
                .unwrap()
        )
    );
    assert_eq!(capability.captured_origin(), facts.origin);
    assert_eq!(capability.id(), facts.id);
    let witness = capability.reopen_witness();
    assert_eq!(witness.document_revision(), revision);
    assert_eq!(witness.table_name(), "Items");
    assert_eq!(witness.column_name(), "Amount");
    same(witness.expected_bytes(), &expected);
    same(&fs::read(&path).unwrap(), &source.bytes);
    let bundle = crate::journal_path(&path);
    assert!(!bundle.exists());
    let saves = SaveWorker::new(|| {});
    let id = saves
        .start_field_addition(capability, bundle.clone())
        .unwrap();
    let report = terminal(&saves);
    assert!(matches!(report.outcome, SaveOutcome::Saved { receipt }
        if receipt.known == SaveKnownOutcome::Committed
        && receipt.schema_change == Some(SaveSchemaChange::ColumnAdded)
        && receipt.matched_rows.is_none() && receipt.changed_rows.is_none()));
    assert!(saves.acknowledge(id));
    same(&fs::read(&path).unwrap(), &expected);
    assert!(!bundle.exists());
    let Response::Opened {
        document_revision: fresh,
        ..
    } = request(&worker, Command::OpenColumnAdded { witness }).unwrap()
    else {
        panic!("expected guarded column reopen");
    };
    assert!(fresh > revision);
    let Response::Data { rows, columns, .. } = request(
        &worker,
        Command::Page {
            table: "Items".into(),
            offset: 0,
        },
    )
    .unwrap() else {
        panic!("expected fresh historical cells");
    };
    assert_eq!(columns[0].native_number, Some(1));
    assert_eq!(columns[1].native_number, Some(2));
    assert_eq!(rows[0][0].text, "23");
    assert!(matches!(
        rows[0][1].kind,
        crate::CellKind::Scalar { is_null: true, .. }
    ));
    let properties = inspect_table_properties(
        &mut Cursor::new(&expected),
        CatalogScope::new(313),
        "Items",
        &mut ResourceBudget::new(u64::MAX, u64::MAX, u64::MAX),
        &|| false,
    )
    .unwrap();
    assert!(
        properties
            .groups()
            .iter()
            .all(|group| !group.name().eq_ignore_ascii_case("Amount"))
    );
    let target = inspect(
        &worker,
        fresh,
        "UPDATE Items SET Amount=CCUR(5.25) WHERE Score=23",
    );
    let Response::QueryUpdateReviewed { review, capability } = request(
        &worker,
        Command::ReviewQueryUpdate {
            target,
            parameters: Vec::new(),
        },
    )
    .unwrap_or_else(|error| panic!("unadorned Currency materialization refused: {error}")) else {
        panic!("expected actual Currency update capability");
    };
    assert_eq!((review.matched_rows, review.changed_rows), (1, 1));
    same(&fs::read(&path).unwrap(), &expected);
    let updated = raw::update_currency(&expected, source.definition, 0, Some(52_500));
    let id = saves
        .start_query_update(capability, crate::journal_path(&path))
        .unwrap();
    let report = terminal(&saves);
    assert!(matches!(report.outcome, SaveOutcome::Saved { receipt }
        if receipt.known == SaveKnownOutcome::Committed
        && receipt.matched_rows == Some(1) && receipt.changed_rows == Some(1)
        && receipt.schema_change.is_none()));
    assert!(saves.acknowledge(id));
    same(&fs::read(&path).unwrap(), &updated);
    assert!(!crate::journal_path(&path).exists());
    let Response::Opened {
        document_revision: fresh,
        ..
    } = request(&worker, crate::tests::open(&path, "")).unwrap()
    else {
        panic!("expected post-update fresh source");
    };
    let observed = read_table(
        &mut Cursor::new(&updated),
        CatalogScope::new(313),
        "Items",
        &mut ResourceBudget::new(u64::MAX, u64::MAX, u64::MAX),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        observed.rows()[0],
        [
            Value::Int32(23),
            Value::Currency(Currency::from_scaled(52_500))
        ]
    );
    assert_eq!(observed.rows()[1], [Value::Null, Value::Null]);
    let unchanged_properties = inspect_table_properties(
        &mut Cursor::new(&updated),
        CatalogScope::new(313),
        "Items",
        &mut ResourceBudget::new(u64::MAX, u64::MAX, u64::MAX),
        &|| false,
    )
    .unwrap();
    assert_eq!(unchanged_properties.raw_bytes(), properties.raw_bytes());
    assert!(
        unchanged_properties
            .groups()
            .iter()
            .all(|group| !group.name().eq_ignore_ascii_case("Amount"))
    );
    let expected = updated;
    let Response::InsertDraft(draft) = request(
        &worker,
        Command::BeginInsert {
            target: InsertTarget {
                document_revision: fresh,
                table: "Items".into(),
            },
        },
    )
    .unwrap() else {
        panic!("expected actual post-ADD insertion metadata");
    };
    assert_eq!(
        draft
            .fields
            .iter()
            .map(|field| field.name.as_str())
            .collect::<Vec<_>>(),
        ["Score", "Amount"]
    );
    assert!(draft.fields.iter().all(|field| !field.auto_number));
    let Response::InsertReviewed { review, capability } = request(
        &worker,
        Command::ReviewInsert {
            draft_id: draft.id,
            inputs: vec![InsertInput::Omitted, InsertInput::Omitted],
        },
    )
    .unwrap_or_else(|error| panic!("post-declared-ADD omitted INSERT refused: {error}")) else {
        panic!("expected the consuming post-ADD omitted review");
    };
    assert_eq!(review.inserted_rows, 1);
    assert!(
        review
            .inputs
            .iter()
            .all(|value| matches!(value, InsertInput::Omitted))
    );
    same(&fs::read(&path).unwrap(), &expected);
    let inserted = raw::append_omitted(&expected, source.definition);
    let id = saves
        .start_insert(capability, crate::journal_path(&path))
        .unwrap();
    let report = terminal(&saves);
    assert!(matches!(report.outcome, SaveOutcome::Saved { receipt }
        if receipt.known == SaveKnownOutcome::Committed
        && receipt.changed_rows == Some(1) && receipt.schema_change.is_none()));
    assert!(saves.acknowledge(id));
    same(&fs::read(&path).unwrap(), &inserted);
    assert!(!crate::journal_path(&path).exists());
    request(&worker, crate::tests::open(&path, "")).unwrap();
    let Response::Data { rows, .. } = request(
        &worker,
        Command::Page {
            table: "Items".into(),
            offset: 0,
        },
    )
    .unwrap() else {
        panic!("expected actual fresh omitted values");
    };
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0][0].text, "23");
    assert!(
        rows[1]
            .iter()
            .all(|cell| matches!(cell.kind, crate::CellKind::Scalar { is_null: true, .. }))
    );
    assert!(
        rows[2]
            .iter()
            .all(|cell| matches!(cell.kind, crate::CellKind::Scalar { is_null: true, .. }))
    );
}

#[test]
fn sql_add_refuses_annotations_and_other_kinds_without_using_gallery_defaults() {
    let source = raw::source();
    let (_directory, path, worker, revision) = opened(&source.bytes);
    for suffix in [
        "CURRENCY NOT NULL",
        "CURRENCY DEFAULT 0",
        "COUNTER",
        "BINARY(16)",
        "TEXT",
    ] {
        let target = inspect(
            &worker,
            revision,
            &format!("ALTER TABLE Items ADD COLUMN Amount {suffix}"),
        );
        assert!(matches!(
            review(&worker, &target),
            Err(Error::SchemaSqlUnsupported)
        ));
        same(&fs::read(&path).unwrap(), &source.bytes);
        assert!(!crate::journal_path(&path).exists());
    }
}

#[test]
fn declared_add_checks_cancel_and_issued_source_before_parsing_or_native_planning() {
    use tablecraft_core::values::ExtendedDateTime;
    use tablecraft_expressions::functions::dates::clock::ClockReading;
    use tablecraft_testkit::support::expressions::clock::CountingClock;

    let source = raw::source();
    let (_directory, path, worker, revision) = opened(&source.bytes);
    let target = inspect(&worker, revision, SQL);
    let (_other, other_path, other_worker, _) = opened(&source.bytes);
    assert!(matches!(
        review(&other_worker, &target),
        Err(Error::StaleQuery)
    ));
    let (mut document, _) =
        crate::document::Document::open(&path, &mut String::new(), 7, &|| false).unwrap();
    let Response::QueryParameters { target: local, .. } =
        document.inspect_query(7, SQL, &|| false).unwrap()
    else {
        panic!("expected actual issued declaration syntax");
    };
    let clock = CountingClock::new(
        ClockReading::new(ExtendedDateTime::new(2026, 11, 11, 12, 0, 0, 0).unwrap()).unwrap(),
    );
    assert_eq!(clock.calls(), 0);
    assert!(matches!(
        document.review_schema_sql(&local, &clock, &|| true),
        Err(Error::Cancelled)
    ));
    assert!(matches!(
        document.review_schema_sql(&target, &clock, &|| false),
        Err(Error::StaleQuery)
    ));
    assert_eq!(
        clock.calls(),
        0,
        "no default expression or clock runs during refusal"
    );
    same(&fs::read(&path).unwrap(), &source.bytes);
    same(&fs::read(&other_path).unwrap(), &source.bytes);
    assert!(!crate::journal_path(&path).exists());
    assert!(!crate::journal_path(&other_path).exists());
}

mod unadorned;

mod multiword;

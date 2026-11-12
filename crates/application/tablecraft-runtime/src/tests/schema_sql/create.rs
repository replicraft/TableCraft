//! Exact SQL declaration images and the existing consuming table-creation service.
mod multiword;
use super::{inspect, review};
use crate::tests::{request, stored::opened, terminal};
use crate::{Command, Error, QueryOperation, Response, SaveOutcome, SaveWorker};
use std::fs;
use tablecraft_testkit::fixtures::access::{
    canonical_blank_creation, canonical_creation, canonical_creation_autonumber,
    canonical_creation_primary,
    canonical_creation_properties::{self, Policy},
    canonical_fixed_creation, canonical_native17_creation,
    canonical_retained_add_table::{self as retained, fields},
    canonical_variable_creation,
};

fn assert_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len(), "complete image length");
    assert_eq!(
        actual
            .iter()
            .zip(expected)
            .position(|(left, right)| left != right),
        None,
        "first different byte"
    );
}

fn save(before: &[u8], expected: &[u8], sql: &str, table: &str, count: usize) {
    let (_directory, path, worker, revision) = opened(before);
    let target = inspect(&worker, revision, sql);
    assert_eq!(target.operation(), QueryOperation::Schema);
    let Response::TableCreationReviewed { capability } = review(&worker, &target).unwrap() else {
        panic!("SQL must return the actual native table-creation capability");
    };
    let witness = capability.reopen_witness();
    assert_eq!(
        witness.draft_revision(),
        None,
        "SQL has no invented design-draft revision"
    );
    assert_eq!(witness.document_revision(), revision);
    assert_eq!(witness.table_name(), table);
    assert_eq!(witness.column_count(), count);
    assert_image(witness.expected_bytes(), expected);
    assert_image(&fs::read(&path).unwrap(), before);
    let bundle = crate::journal_path(&path);
    assert!(!bundle.exists());
    let saves = SaveWorker::new(|| {});
    let id = saves
        .start_table_creation(capability, bundle.clone())
        .unwrap();
    let report = terminal(&saves);
    assert_eq!(report.id, id);
    assert!(
        matches!(report.outcome, SaveOutcome::Saved { .. }),
        "{report:?}"
    );
    assert!(saves.acknowledge(id));
    assert!(!bundle.exists());
    assert_image(&fs::read(&path).unwrap(), expected);
    let Response::Opened {
        document_revision: fresh,
        tables,
        ..
    } = request(&worker, Command::OpenTableCreated { witness }).unwrap()
    else {
        panic!("expected guarded complete-image creation publication");
    };
    assert!(fresh > revision);
    assert!(tables.iter().any(|entry| entry.name == table));
    assert!(matches!(review(&worker, &target), Err(Error::StaleQuery)));
    let Response::Data {
        columns,
        rows,
        has_more,
        ..
    } = request(
        &worker,
        Command::Page {
            table: table.into(),
            offset: 0,
        },
    )
    .unwrap()
    else {
        panic!("expected the newly created native table");
    };
    assert_eq!(columns.len(), count);
    assert!(rows.is_empty() && !has_more);
    assert_image(&fs::read(path).unwrap(), expected);
}

#[test]
fn sql_create_manual_keyless_named_primary_and_explicit_counter_match_independent_images() {
    let blank = canonical_blank_creation::source();
    let keyless = canonical_native17_creation::source(
        canonical_creation::source(canonical_creation::Case::Empty).bytes,
    );
    save(
        &blank,
        &keyless,
        "CREATE TABLE Items (Id INTEGER, Score LONG)",
        "Items",
        2,
    );
    let keyed = canonical_creation_primary::source(canonical_creation_primary::Case::Empty).bytes;
    for sql in [
        "CREATE TABLE Items (Id INT CONSTRAINT PrimaryId PRIMARY KEY, Score INTEGER4)",
        "CREATE TABLE Items (Id LONG, Score INTEGER, CONSTRAINT PrimaryId PRIMARY KEY ([iD]))",
    ] {
        save(&blank, &keyed, sql, "Items", 2);
    }
    let generated =
        canonical_creation_autonumber::source(canonical_creation_autonumber::Case::EmptyPrimary)
            .bytes;
    save(
        &blank,
        &generated,
        "CREATE TABLE Items (Id COUNTER CONSTRAINT PrimaryId PRIMARY KEY, Score LONG)",
        "Items",
        2,
    );
}

#[test]
fn sql_fixed_and_variable_aliases_preserve_exact_widths_and_absent_property_defaults() {
    let blank = canonical_blank_creation::source();
    let fixed = canonical_native17_creation::source(
        canonical_fixed_creation::source(canonical_fixed_creation::Case::Empty).bytes,
    );
    save(
        &blank,
        &fixed,
        "CREATE TABLE Items (Flag LOGICAL1, ByteValue INTEGER1, Int16Value INTEGER2, Int32Value INTEGER4, Int64Value BIGINT, [Money] MONEY, SingleValue FLOAT4, DoubleValue FLOAT8, DateValue DATE, GuidValue UNIQUEIDENTIFIER)",
        "Items",
        10,
    );
    let variable = canonical_native17_creation::source(
        canonical_variable_creation::source(canonical_variable_creation::Case::Empty).bytes,
    );
    save(
        &blank,
        &variable,
        "CREATE TABLE Items (Id INTEGER, TextValue NCHAR(8), BinaryValue VARBINARY(16), DecimalValue DEC(28,4), Flag YESNO)",
        "Items",
        5,
    );
    let required = canonical_creation_properties::with_policies(
        variable,
        &[Policy {
            column: "Flag",
            required: Some(true),
            allow_zero_length: None,
        }],
    );
    save(
        &blank,
        &required,
        "CREATE TABLE Items (Id LONG, TextValue TEXT(8), BinaryValue BINARY(16), DecimalValue DECIMAL(28,4), Flag BIT NOT NULL)",
        "Items",
        5,
    );
}

#[test]
fn sql_create_beside_packed_and_retained_tables_preserves_every_independent_sibling_byte() {
    let packed = canonical_native17_creation::source(
        canonical_creation::source(canonical_creation::Case::Empty).bytes,
    );
    let retained = fields::source(fields::Case::FirstNumber).bytes;
    for before in [packed, retained] {
        let expected = retained::append(&before, "Added", retained::Target::KeylessNumber);
        save(
            &before,
            &expected,
            "CREATE TABLE Added (Id LONG, Score INTEGER)",
            "Added",
            2,
        );
    }
}

#[test]
fn sql_create_refuses_unqualified_declarations_and_existing_names_without_effects() {
    let blank = canonical_blank_creation::source();
    let (_directory, path, worker, revision) = opened(&blank);
    for sql in [
        "CREATE TEMPORARY TABLE Added (Id LONG)",
        "CREATE TABLE Added (Id LONG DEFAULT 0)",
        "CREATE TABLE Added (Id LONG DEFAULT NULL)",
        "CREATE TABLE Added (Label TEXT(20) DEFAULT Unknown)",
        "CREATE TABLE Added (Label TEXT)",
        "CREATE TABLE Added (Label CHAR(20) WITH COMPRESSION)",
        "CREATE TABLE Added (Id LONG PRIMARY KEY)",
        "CREATE TABLE Added (Id LONG, CONSTRAINT Many PRIMARY KEY (Id,Id))",
        "CREATE TABLE Added (Id LONG, CONSTRAINT Missing PRIMARY KEY (Other))",
        "CREATE TABLE Added (Id LONG UNIQUE)",
        "CREATE TABLE Added (Id LONG CHECK (Id > 0))",
        "CREATE TABLE Added (Id LONG REFERENCES Other (Id))",
        "CREATE TABLE Added (Payload IMAGE)",
        "CREATE TABLE Added (WhenValue DATETIMEEXTENDED)",
    ] {
        let target = inspect(&worker, revision, sql);
        assert!(
            matches!(review(&worker, &target), Err(Error::SchemaSqlUnsupported)),
            "{sql}"
        );
        assert_image(&fs::read(&path).unwrap(), &blank);
        assert!(!crate::journal_path(&path).exists());
    }
    for sql in [
        "CREATE TABLE Added (Id COUNTER)",
        "CREATE TABLE Added (Label TEXT(256))",
        "CREATE TABLE Added (Id LONG, [id] LONG)",
    ] {
        let target = inspect(&worker, revision, sql);
        assert!(
            matches!(review(&worker, &target), Err(Error::Creation(_))),
            "{sql}"
        );
        assert_image(&fs::read(&path).unwrap(), &blank);
    }
    let packed = canonical_native17_creation::source(
        canonical_creation::source(canonical_creation::Case::Empty).bytes,
    );
    let (_directory, path, worker, revision) = opened(&packed);
    let target = inspect(
        &worker,
        revision,
        "CREATE TABLE [iTeMs] (Id LONG, Score LONG)",
    );
    assert!(matches!(review(&worker, &target), Err(Error::AddTable(_))));
    assert_image(&fs::read(&path).unwrap(), &packed);
    assert!(!crate::journal_path(&path).exists());
}

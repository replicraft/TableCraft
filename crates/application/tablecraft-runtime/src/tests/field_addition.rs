//! Public generic field review/save preserves exact declarations and older Number sources.
use super::{open, request, stored::opened, terminal};
use crate::{
    Command, Error, FieldAdditionAvailability, FieldAdditionReview, Response,
    ReviewedFieldAddition, SaveOutcome, SaveSchemaChange, SaveStartFailure, SaveWorker,
};
use std::num::NonZeroU8;
use tablecraft_builders::table::FieldAdditionDeclaration as Declaration;
use tablecraft_testkit::fixtures::access::canonical_created_number::mixed as raw;

fn text(width: u8, compression: bool) -> Declaration {
    Declaration::ShortText {
        max_utf16_units: NonZeroU8::new(width).unwrap(),
        unicode_compression: compression,
    }
}

fn inspect(worker: &crate::Worker, revision: u64) -> FieldAdditionAvailability {
    let Response::FieldAdditionAvailability { availability } = request(
        worker,
        Command::InspectFieldAdditionAvailability {
            document_revision: revision,
            table: "Table1".into(),
        },
    )
    .unwrap() else {
        panic!("expected source-bound per-kind facts");
    };
    assert_eq!(availability.document_revision, revision);
    assert_eq!(availability.table, "Table1");
    availability
}

fn review(
    worker: &crate::Worker,
    revision: u64,
    column: &str,
    declaration: Declaration,
) -> (FieldAdditionReview, ReviewedFieldAddition) {
    let Response::FieldAdditionReviewed { review, capability } = request(
        worker,
        Command::ReviewAddField {
            document_revision: revision,
            table: "Table1".into(),
            column: column.into(),
            declaration,
        },
    )
    .unwrap() else {
        panic!("expected complete declaration-bound capability");
    };
    assert_eq!(review.document_revision, revision);
    assert_eq!(review.table, "Table1");
    assert_eq!(review.column, column);
    assert_eq!(review.declaration, declaration);
    assert_eq!(capability.captured_declaration(), declaration);
    (review, capability)
}

#[test]
fn exact_width_and_compression_are_derived_from_each_captured_native_plan() {
    let before = raw::source(raw::SourceCase::IdOnly);
    let (_directory, path, worker, revision) = opened(&before);
    for (width, compression) in [(1, true), (80, true), (255, true), (255, false)] {
        let (_, capability) = review(&worker, revision, "Label", text(width, compression));
        assert_eq!(
            capability.reopen_witness().expected_bytes(),
            raw::append_text(&before, "Label", u16::from(width), compression)
        );
    }
    assert_eq!(std::fs::read(path).unwrap(), before);
}

#[test]
fn text_then_number_uses_one_real_save_worker_and_exact_full_witness_reopens() {
    let before = raw::source(raw::SourceCase::IdOnly);
    let label = raw::append_text(&before, "Label", 255, true);
    let expected = raw::append_number(&label, "AfterLabel");
    let (directory, path, worker, mut revision) = opened(&before);
    let saves = SaveWorker::new(|| {});
    for (column, declaration, after) in [
        ("Label", text(255, true), &label),
        ("AfterLabel", Declaration::NumberLongZero, &expected),
    ] {
        let facts = inspect(&worker, revision);
        assert!(facts.number_available && facts.short_text_available);
        let (_, capability) = review(&worker, revision, column, declaration);
        let witness = capability.reopen_witness();
        assert_eq!(witness.expected_bytes(), after);
        let bundle = directory.path().join(format!("{column}.journal"));
        let id = saves
            .start_field_addition(capability, bundle.clone())
            .unwrap();
        let report = terminal(&saves);
        let SaveOutcome::Saved { receipt } = report.outcome else {
            panic!("expected real native field save: {report:?}");
        };
        assert_eq!(receipt.schema_change, Some(SaveSchemaChange::ColumnAdded));
        assert_eq!((receipt.matched_rows, receipt.changed_rows), (None, None));
        assert!(receipt.query_change.is_none() && receipt.database_options_change.is_none());
        assert!(
            report
                .outcome
                .field_addition_reopen_facts()
                .unwrap()
                .replacement
        );
        assert!(saves.acknowledge(id));
        assert!(!bundle.exists());
        assert_eq!(std::fs::read(&path).unwrap(), *after);
        let Response::Opened {
            document_revision, ..
        } = request(&worker, Command::OpenColumnAdded { witness }).unwrap()
        else {
            panic!("expected source-qualified fresh catalog");
        };
        assert_ne!(document_revision, revision);
        revision = document_revision;
    }
}

#[test]
fn old_number_creator_prefix_keeps_number_support_alongside_qualified_text() {
    use tablecraft_ace::{CreateColumn, create_ace16_database};
    use tablecraft_schema::ScalarType;
    let id = CreateColumn::new("ID", ScalarType::Int32)
        .with_primary_key("PrimaryKey")
        .unwrap()
        .with_incrementing_autonumber()
        .unwrap();
    let byte = CreateColumn::new("Flag", ScalarType::Byte);
    let before = create_ace16_database(
        "Table1",
        &[id, byte],
        &[],
        &mut crate::controls::budget(),
        &|| false,
    )
    .unwrap();
    let (directory, path, worker, revision) = opened(before.as_bytes());
    let facts = inspect(&worker, revision);
    assert!(facts.number_available);
    assert!(facts.short_text_available);
    let (_, capability) = review(&worker, revision, "Score", Declaration::NumberLongZero);
    let witness = capability.reopen_witness();
    let expected = witness.expected_bytes().to_vec();
    let saves = SaveWorker::new(|| {});
    let id = saves
        .start_field_addition(capability, directory.path().join("legacy-number.journal"))
        .unwrap();
    assert!(matches!(
        terminal(&saves).outcome,
        SaveOutcome::Saved { .. }
    ));
    assert!(saves.acknowledge(id));
    assert_eq!(std::fs::read(path).unwrap(), expected);
    request(&worker, Command::OpenColumnAdded { witness }).unwrap();
}

#[test]
fn generic_review_revision_and_consuming_owner_staleness_never_creates_a_journal() {
    let before = raw::source(raw::SourceCase::IdOnly);
    let (directory, path, worker, revision) = opened(&before);
    let (_, capability) = review(&worker, revision, "Label", text(255, true));
    request(&worker, open(&path, "")).unwrap();
    let saves = SaveWorker::new(|| {});
    let bundle = directory.path().join("stale.journal");
    let error = saves
        .start_field_addition(capability, bundle.clone())
        .unwrap_err();
    assert_eq!(error.kind, SaveStartFailure::StaleDocument);
    assert_eq!(
        error.into_reviewed().unwrap().captured_declaration(),
        text(255, true)
    );
    assert!(!bundle.exists());
    assert!(matches!(
        request(
            &worker,
            Command::ReviewAddField {
                document_revision: revision,
                table: "Table1".into(),
                column: "Label".into(),
                declaration: text(255, true)
            }
        ),
        Err(Error::StaleDocument)
    ));
    assert_eq!(std::fs::read(path).unwrap(), before);
}

#[test]
fn largest_observed_number_history_inspects_both_kinds_under_actual_runtime_budget() {
    use tablecraft_testkit::fixtures::access::canonical_created_number::{Source, repeated};
    let mut before = repeated::source(Source::IdOnly);
    for number in 2..=13 {
        before = repeated::append(&before, &format!("N{number}"));
    }
    let (_directory, path, worker, revision) = opened(&before);
    let facts = inspect(&worker, revision);
    assert!(facts.number_available && facts.short_text_available);
    let (_, capability) = review(&worker, revision, "N14", Declaration::NumberLongZero);
    assert_eq!(
        capability.reopen_witness().expected_bytes(),
        repeated::append(&before, "N14")
    );
    assert_eq!(std::fs::read(path).unwrap(), before);
}

#[test]
fn broader_prefix_policies_select_typed_number_before_qualification() {
    use tablecraft_ace::{CreateColumn, create_ace16_database};
    use tablecraft_core::values::Value;
    use tablecraft_schema::ScalarType;
    let id = CreateColumn::new("ID", ScalarType::Int32)
        .with_primary_key("PrimaryKey")
        .unwrap()
        .with_incrementing_autonumber()
        .unwrap();
    let default = Value::Byte(7);
    let field = CreateColumn::new("Flag", ScalarType::Byte)
        .with_required(true)
        .with_default_literal(&default)
        .unwrap()
        .with_description("flag")
        .unwrap();
    let before = create_ace16_database(
        "Table1",
        &[id, field],
        &[],
        &mut crate::controls::budget(),
        &|| false,
    )
    .unwrap();
    let (_directory, path, worker, revision) = opened(before.as_bytes());
    let facts = inspect(&worker, revision);
    assert!(facts.number_available && facts.short_text_available);
    let (_, capability) = review(&worker, revision, "Score", Declaration::NumberLongZero);
    assert_eq!(
        capability.captured_declaration(),
        Declaration::NumberLongZero
    );
    assert_eq!(std::fs::read(path).unwrap(), before.as_bytes());
}

mod after_off;

mod after_off_history;

mod typed;

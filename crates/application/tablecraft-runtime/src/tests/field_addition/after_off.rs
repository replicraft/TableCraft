//! Normal field Save consumes TCAC6 only after actual OFF source selection.
use super::*;
use tablecraft_testkit::fixtures::access::{
    canonical_created_number::{Source as NumberSource, mixed::after_off as raw, repeated},
    canonical_database_options,
};

#[test]
fn four_sdk_pinned_saves_use_normal_field_save_and_full_witness_reopen() {
    for case in [raw::SourceCase::Number, raw::SourceCase::Mixed] {
        let before = raw::source(case);
        for (column, declaration, after) in [
            (
                "AfterOff",
                Declaration::NumberLongZero,
                raw::append_number(&before, "AfterOff"),
            ),
            (
                "LabelOff",
                text(255, true),
                raw::append_text(&before, "LabelOff"),
            ),
        ] {
            let (directory, path, worker, revision) = opened(&before);
            let facts = inspect(&worker, revision);
            assert!(facts.number_available && facts.short_text_available);
            let (_, capability) = review(&worker, revision, column, declaration);
            let witness = capability.reopen_witness();
            assert_eq!(witness.expected_bytes(), after);
            assert_eq!(std::fs::read(&path).unwrap(), before);
            let bundle = directory.path().join("after-off.journal");
            let saves = SaveWorker::new(|| {});
            let id = saves
                .start_field_addition(capability, bundle.clone())
                .unwrap();
            let report = terminal(&saves);
            let SaveOutcome::Saved { receipt } = report.outcome else {
                panic!("expected TCAC6 save: {report:?}");
            };
            assert_eq!(receipt.schema_change, Some(SaveSchemaChange::ColumnAdded));
            assert_eq!((receipt.matched_rows, receipt.changed_rows), (None, None));
            assert!(receipt.database_options_change.is_none());
            assert!(receipt.query_change.is_none());
            assert!(saves.acknowledge(id));
            assert!(!bundle.exists());
            assert_eq!(std::fs::read(&path).unwrap(), after);
            let Response::Opened {
                document_revision, ..
            } = request(&worker, Command::OpenColumnAdded { witness }).unwrap()
            else {
                panic!("expected exact witness reopen");
            };
            assert_ne!(document_revision, revision);
            // The current later-event profile stays outside the settled source
            // language. Selection never retries a legacy plan after refusal.
            let facts = inspect(&worker, document_revision);
            assert!(!facts.number_available && !facts.short_text_available);
        }
    }
}

#[test]
fn after_off_review_refuses_unobserved_targets_without_a_write_or_journal() {
    let before = raw::source(raw::SourceCase::Number);
    let (_directory, path, worker, revision) = opened(&before);
    for declaration in [text(80, true), text(255, false)] {
        assert!(matches!(
            request(
                &worker,
                Command::ReviewAddField {
                    document_revision: revision,
                    table: "Table1".into(),
                    column: "LabelOff".into(),
                    declaration,
                }
            ),
            Err(Error::FieldAdditionAfterOff(
                tablecraft_ace::FieldAdditionAfterOffIntentError::Field(
                    tablecraft_ace::FieldAdditionAfterOffError::Target
                )
            ))
        ));
    }
    assert_eq!(std::fs::read(path).unwrap(), before);
}

#[test]
fn insufficient_room_is_a_source_fact_and_shorter_named_fit_remains_usable() {
    let mut before = repeated::source(NumberSource::IdOnly);
    for ordinal in 2..=12 {
        before = repeated::append(&before, &format!("N{ordinal}"));
    }
    let current = canonical_database_options::append_explicit_off_to_owned_page(&before);
    let (_directory, path, worker, revision) = opened(&current);
    let facts = inspect(&worker, revision);
    assert!(!facts.number_available && !facts.short_text_available);
    assert_eq!(std::fs::read(path).unwrap(), current);

    let mut before = repeated::source(NumberSource::IdOnly);
    for ordinal in 0..5 {
        before = repeated::append(&before, &format!("N{ordinal}{}", "x".repeat(60)));
    }
    let current = canonical_database_options::append_explicit_off_to_owned_page(&before);
    let (_directory, path, worker, revision) = opened(&current);
    assert!(inspect(&worker, revision).number_available);
    let name = "Z".repeat(59);
    let (_, capability) = review(&worker, revision, &name, Declaration::NumberLongZero);
    assert_eq!(
        capability.reopen_witness().expected_bytes(),
        raw::append_number(&current, &name)
    );
    assert!(
        request(
            &worker,
            Command::ReviewAddField {
                document_revision: revision,
                table: "Table1".into(),
                column: "Z".repeat(60),
                declaration: Declaration::NumberLongZero,
            }
        )
        .is_err()
    );
    assert_eq!(std::fs::read(path).unwrap(), current);
}

#[test]
fn after_off_capability_obeys_existing_revision_and_consumer_staleness() {
    let before = raw::source(raw::SourceCase::Number);
    let (directory, path, worker, revision) = opened(&before);
    let (_, capability) = review(&worker, revision, "AfterOff", Declaration::NumberLongZero);
    request(&worker, open(&path, "")).unwrap();
    let saves = SaveWorker::new(|| {});
    let bundle = directory.path().join("stale.journal");
    let error = saves
        .start_field_addition(capability, bundle.clone())
        .unwrap_err();
    assert_eq!(error.kind, SaveStartFailure::StaleDocument);
    assert_eq!(
        error.into_reviewed().unwrap().captured_declaration(),
        Declaration::NumberLongZero
    );
    assert!(!bundle.exists());
    assert_eq!(std::fs::read(path).unwrap(), before);
}

//! Existing normal field actor/witness with explicit chronological source purpose.
use super::*;
use crate::FieldAdditionOrigin;
use tablecraft_testkit::fixtures::access::canonical_created_number::mixed::after_off::{
    self as raw, history,
};

#[test]
fn four_second_add_saves_match_independent_images_and_preserve_off_receipt_scope() {
    for case in [
        history::SourceCase::NumberThenNumber,
        history::SourceCase::MixedThenText,
    ] {
        let before = history::source(case);
        for (column, declaration, expected) in [
            (
                "SecondNumber",
                Declaration::NumberLongZero,
                raw::append_number(&before, "SecondNumber"),
            ),
            (
                "SecondLabel",
                text(255, true),
                raw::append_text(&before, "SecondLabel"),
            ),
        ] {
            let (directory, path, worker, revision) = opened(&before);
            let facts = inspect(&worker, revision);
            assert!(facts.number_available && facts.short_text_available);
            let (_, capability) = review(&worker, revision, column, declaration);
            let witness = capability.reopen_witness();
            assert_eq!(witness.expected_bytes(), expected);
            let bundle = directory.path().join("history.journal");
            let saves = SaveWorker::new(|| {});
            let id = saves
                .start_field_addition(capability, bundle.clone())
                .unwrap();
            let report = terminal(&saves);
            let SaveOutcome::Saved { receipt } = report.outcome else {
                panic!("expected real history save: {report:?}");
            };
            assert_eq!(receipt.schema_change, Some(SaveSchemaChange::ColumnAdded));
            assert_eq!((receipt.matched_rows, receipt.changed_rows), (None, None));
            assert!(receipt.query_change.is_none() && receipt.database_options_change.is_none());
            assert!(saves.acknowledge(id));
            assert!(!bundle.exists());
            assert_eq!(std::fs::read(&path).unwrap(), expected);
            let Response::Opened {
                document_revision, ..
            } = request(&worker, Command::OpenColumnAdded { witness }).unwrap()
            else {
                panic!("expected full witness publication");
            };
            assert_ne!(document_revision, revision);
            let facts = inspect(&worker, document_revision);
            assert!(facts.number_available && facts.short_text_available);
        }
    }
}

#[test]
fn second_then_raw_third_field_use_one_actor_and_fresh_revision_bound_reviews() {
    let before = history::source(history::SourceCase::NumberThenNumber);
    let second = raw::append_text(&before, "SecondLabel");
    let third = raw::append_number(&second, "ThirdNumber");
    let (directory, path, worker, mut revision) = opened(&before);
    let saves = SaveWorker::new(|| {});
    for (column, declaration, expected) in [
        ("SecondLabel", text(255, true), second),
        ("ThirdNumber", Declaration::NumberLongZero, third),
    ] {
        let (_, capability) = review(&worker, revision, column, declaration);
        let witness = capability.reopen_witness();
        assert_eq!(witness.expected_bytes(), expected);
        let id = saves
            .start_field_addition(
                capability,
                directory.path().join(format!("{column}.journal")),
            )
            .unwrap();
        assert!(matches!(
            terminal(&saves).outcome,
            SaveOutcome::Saved { .. }
        ));
        assert!(saves.acknowledge(id));
        assert_eq!(std::fs::read(&path).unwrap(), expected);
        let Response::Opened {
            document_revision, ..
        } = request(&worker, Command::OpenColumnAdded { witness }).unwrap()
        else {
            panic!("expected checked reopen");
        };
        assert_ne!(document_revision, revision);
        revision = document_revision;
    }
    // The third event is raw-model evidence, not another SDK execution claim.
}

#[test]
fn history_review_refuses_unobserved_declarations_and_stale_capabilities() {
    let before = history::source(history::SourceCase::NumberThenNumber);
    let (directory, path, worker, revision) = opened(&before);
    for declaration in [text(80, true), text(255, false)] {
        assert!(matches!(
            request(
                &worker,
                Command::ReviewAddField {
                    document_revision: revision,
                    table: "Table1".into(),
                    column: "Next".into(),
                    declaration
                }
            ),
            Err(Error::FieldAdditionAfterOffHistory(
                tablecraft_ace::FieldAdditionAfterOffHistoryIntentError::Field(
                    tablecraft_ace::FieldAdditionAfterOffHistoryError::Field(
                        tablecraft_ace::FieldAdditionAfterOffError::Target
                    )
                )
            ))
        ));
    }
    let (_, capability) = review(
        &worker,
        revision,
        "SecondNumber",
        Declaration::NumberLongZero,
    );
    request(&worker, open(&path, "")).unwrap();
    let saves = SaveWorker::new(|| {});
    let bundle = directory.path().join("stale-history.journal");
    let failure = saves
        .start_field_addition(capability, bundle.clone())
        .unwrap_err();
    assert_eq!(failure.kind, SaveStartFailure::StaleDocument);
    assert_eq!(
        failure.into_reviewed().unwrap().captured_origin(),
        FieldAdditionOrigin::Gallery(Declaration::NumberLongZero)
    );
    assert!(!bundle.exists());
    assert_eq!(std::fs::read(path).unwrap(), before);
}

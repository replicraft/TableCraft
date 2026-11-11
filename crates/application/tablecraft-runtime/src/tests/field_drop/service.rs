//! Consuming native DROP actors and complete-image reopen at real file boundaries.
mod publication;
use super::super::{open, request, stored::opened, terminal};
use crate::{
    Command, Error, FieldDropReview, Response, ReviewedFieldDrop, SaveFailureKind, SaveOutcome,
    SaveSchemaChange, SaveStartFailure, SaveWorker,
};
use tablecraft_testkit::fixtures::access::canonical_created_number::mixed::after_off::deleted_field as raw;

fn review(
    worker: &crate::Worker,
    revision: u64,
    column: &str,
    native_number: usize,
) -> Result<(FieldDropReview, ReviewedFieldDrop), Error> {
    let response = request(
        worker,
        Command::ReviewFieldDrop {
            document_revision: revision,
            table: "Table1".into(),
            column: column.into(),
            native_number,
        },
    )?;
    let Response::FieldDropReviewed { review, capability } = response else {
        panic!("expected an actual consuming saved-field DROP review");
    };
    assert_eq!(review.document_revision, revision);
    assert_eq!(review.table, capability.captured_table());
    assert_eq!(review.column, capability.captured_column());
    assert_eq!(review.native_number, capability.captured_native_number());
    Ok((review, capability))
}

#[test]
fn number_and_text_drop_use_one_actor_and_the_complete_independent_cleanup_after() {
    for (case, column, survivors) in [
        (
            raw::SourceCase::NumberHistory,
            "Score",
            vec![("ID", 0), ("Score2", 2)],
        ),
        (
            raw::SourceCase::MixedTextHistory,
            "Label",
            vec![("ID", 0), ("AfterLabel", 2), ("OtherLabel", 3)],
        ),
    ] {
        let before = raw::source(case);
        let after = raw::remove_nonprimary(&before, column, raw::Variant::AppendedGroupRemoved);
        let (directory, path, worker, revision) = opened(&before);
        let (facts, capability) = review(&worker, revision, column, 1).unwrap();
        let (_, second) = review(&worker, revision, column, 1).unwrap();
        assert_eq!(facts.column, column);
        assert_eq!(facts.native_number, 1);
        let witness = capability.reopen_witness();
        assert_eq!(witness.expected_bytes(), after);
        assert_eq!(witness.table_name(), "Table1");
        assert_eq!(witness.original_column_name(), column);
        assert_eq!(witness.native_number(), 1);
        // Neither inspection nor the consuming review holds a target lock while
        // a host waits for its own explicit destructive confirmation.
        tablecraft_access::editing::offline::OfflineWriteSession::open(
            &path,
            crate::controls::SCOPE,
            &mut crate::controls::budget(),
            &|| false,
        )
        .unwrap()
        .release()
        .unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), before);

        let saves = SaveWorker::new(|| {});
        assert!(SaveWorker::supports_field_drop());
        let bundle = directory.path().join("drop.journal");
        let id = saves.start_field_drop(capability, bundle.clone()).unwrap();
        let busy = saves
            .start_field_drop(second, directory.path().join("second.journal"))
            .unwrap_err();
        assert_eq!(busy.kind, SaveStartFailure::Busy);
        assert!(busy.into_reviewed().is_some());
        let report = terminal(&saves);
        let SaveOutcome::Saved { receipt } = report.outcome else {
            panic!("expected actual DROP completion: {report:?}");
        };
        assert_eq!(report.id, id);
        assert_eq!(receipt.schema_change, Some(SaveSchemaChange::ColumnDropped));
        assert_eq!((receipt.matched_rows, receipt.changed_rows), (None, None));
        assert!(receipt.query_change.is_none());
        assert!(receipt.database_options_change.is_none());
        assert!(
            report
                .outcome
                .field_drop_reopen_facts()
                .unwrap()
                .replacement
        );
        assert!(saves.acknowledge(id));
        assert!(!bundle.exists());
        assert_eq!(std::fs::read(&path).unwrap(), after);

        let Response::Opened {
            document_revision: fresh,
            ..
        } = request(&worker, Command::OpenFieldDropped { witness }).unwrap()
        else {
            panic!("expected a complete DROP witness reopen");
        };
        assert!(fresh > revision);
        let Response::Data { columns, rows, .. } = request(
            &worker,
            Command::Page {
                table: "Table1".into(),
                offset: 0,
            },
        )
        .unwrap() else {
            panic!("expected fresh native survivor declarations");
        };
        assert!(rows.is_empty());
        assert_eq!(columns.len(), survivors.len());
        for (field, (name, number)) in columns.iter().zip(survivors) {
            assert_eq!(field.name, name);
            assert_eq!(field.native_number, Some(number));
        }
    }
}

#[test]
fn exact_target_revision_and_stale_consuming_review_refuse_before_an_actor_or_journal() {
    let before = raw::source(raw::SourceCase::NumberHistory);
    let (directory, path, worker, revision) = opened(&before);
    assert!(matches!(
        review(&worker, revision + 1, "Score", 1),
        Err(Error::StaleDocument)
    ));
    for (column, number) in [("score", 1), ("Score", 0), ("Score", 99)] {
        assert!(matches!(
            review(&worker, revision, column, number),
            Err(Error::StaleFieldDropTarget)
        ));
    }
    let (_, capability) = review(&worker, revision, "Score", 1).unwrap();
    request(&worker, open(&path, "")).unwrap();
    let saves = SaveWorker::new(|| {});
    let bundle = directory.path().join("stale.journal");
    let failure = saves
        .start_field_drop(capability, bundle.clone())
        .unwrap_err();
    assert_eq!(failure.kind, SaveStartFailure::StaleDocument);
    assert!(failure.into_reviewed().is_some());
    assert!(saves.poll().is_none());
    assert!(!bundle.exists());
    assert_eq!(std::fs::read(path).unwrap(), before);
}

#[test]
fn actual_after_ack_still_requires_a_full_witness_and_cancelled_read_never_writes() {
    let before = raw::source(raw::SourceCase::NumberHistory);
    let (directory, path, worker, revision) = opened(&before);
    let (_, capability) = review(&worker, revision, "Score", 1).unwrap();
    let witness = capability.reopen_witness();
    let saves = SaveWorker::new(|| {});
    let bundle = directory.path().join("after.journal");
    let id = saves.start_field_drop(capability, bundle.clone()).unwrap();
    assert!(matches!(
        terminal(&saves).outcome,
        SaveOutcome::Saved { .. }
    ));
    assert!(saves.acknowledge(id));
    assert!(!bundle.exists());
    assert!(matches!(
        crate::document::Document::open_field_dropped(&witness, revision + 1, &|| true),
        Err(Error::Cancelled)
    ));
    let mut foreign = std::fs::read(&path).unwrap();
    *foreign.last_mut().unwrap() ^= 1;
    std::fs::write(&path, &foreign).unwrap();
    assert!(matches!(
        request(&worker, Command::OpenFieldDropped { witness }),
        Err(Error::FieldDroppedSourceChanged)
    ));
    assert_eq!(std::fs::read(path).unwrap(), foreign);
}

#[test]
fn changed_source_after_review_retains_the_real_release_owner_without_a_journal() {
    let before = raw::source(raw::SourceCase::NumberHistory);
    let (directory, path, worker, revision) = opened(&before);
    let (_, capability) = review(&worker, revision, "Score", 1).unwrap();
    let mut foreign = before;
    foreign[7 * 4096 + 3000] ^= 1;
    std::fs::write(&path, &foreign).unwrap();
    let saves = SaveWorker::new(|| {});
    let bundle = directory.path().join("refused.journal");
    let id = saves.start_field_drop(capability, bundle.clone()).unwrap();
    let SaveOutcome::Failed(failure) = terminal(&saves).outcome else {
        panic!("changed source must retain a refusal owner");
    };
    assert_eq!(failure.kind, SaveFailureKind::SourceChanged);
    assert!(failure.can_release && !failure.can_recover);
    assert!(!bundle.exists());
    assert!(!saves.acknowledge(id));
    let release = saves.release(id).unwrap();
    assert!(matches!(
        terminal(&saves).outcome,
        SaveOutcome::Released { .. }
    ));
    assert!(saves.acknowledge(release));
    assert_eq!(std::fs::read(path).unwrap(), foreign);
}

mod inserting;

mod typed;

mod non_tail;

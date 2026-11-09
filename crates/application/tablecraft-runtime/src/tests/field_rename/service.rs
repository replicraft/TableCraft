//! Actual reviewed field saves, exact native metadata and complete-image reads.
use super::super::{open, request, stored::opened, terminal};
use crate::{
    Command, Error, FieldRenameReview, Response, ReviewedFieldRename, SaveFailureKind, SaveOutcome,
    SaveSchemaChange, SaveStartFailure, SaveWorker,
};
use tablecraft_testkit::fixtures::access::{
    canonical_created_number::mixed::after_off as raw, saved_field_rename,
};

fn review(
    worker: &crate::Worker,
    revision: u64,
    column: &str,
    native_number: usize,
    new_name: &str,
) -> Result<(FieldRenameReview, ReviewedFieldRename), Error> {
    let response = request(
        worker,
        Command::ReviewFieldRename {
            document_revision: revision,
            table: "Table1".into(),
            column: column.into(),
            native_number,
            new_name: new_name.into(),
        },
    )?;
    let Response::FieldRenameReviewed { review, capability } = response else {
        panic!("expected a genuine original-request field rename review");
    };
    assert_eq!(review.document_revision, revision);
    assert_eq!(review.table, capability.captured_table());
    assert_eq!(review.column, capability.captured_column());
    assert_eq!(review.native_number, capability.captured_native_number());
    assert_eq!(review.new_name, capability.captured_new_name());
    assert_eq!(review.changed, capability.changed());
    Ok((review, capability))
}

#[test]
fn native_number_and_text_renames_use_one_actor_and_exact_independent_after_witness() {
    for (case, old, new) in [
        (raw::SourceCase::Number, "Score", "Total"),
        (raw::SourceCase::Mixed, "Label", "Title"),
    ] {
        let [before, expected] = saved_field_rename::pair(case);
        let (directory, path, worker, revision) = opened(&before);
        let (facts, capability) = review(&worker, revision, old, 1, new).unwrap();
        let (_, second) = review(&worker, revision, old, 1, new).unwrap();
        assert_eq!(
            (
                facts.column.as_str(),
                facts.native_number,
                facts.new_name.as_str()
            ),
            (old, 1, new)
        );
        let witness = capability.reopen_witness();
        assert_eq!(witness.expected_bytes(), expected);
        // Review holds no exclusive target lease and has not changed any bytes.
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
        let bundle = directory.path().join("rename.journal");
        let id = saves
            .start_field_rename(capability, bundle.clone())
            .unwrap();
        let busy = saves
            .start_field_rename(second, directory.path().join("second.journal"))
            .unwrap_err();
        assert_eq!(busy.kind, SaveStartFailure::Busy);
        assert!(busy.into_reviewed().is_some());
        let report = terminal(&saves);
        let SaveOutcome::Saved { receipt } = report.outcome else {
            panic!("expected actual native field rename completion: {report:?}");
        };
        assert_eq!(report.id, id);
        assert_eq!(receipt.schema_change, Some(SaveSchemaChange::ColumnRenamed));
        assert_eq!((receipt.matched_rows, receipt.changed_rows), (None, None));
        assert!(receipt.query_change.is_none() && receipt.database_options_change.is_none());
        assert!(
            report
                .outcome
                .field_rename_reopen_facts()
                .unwrap()
                .replacement
        );
        assert!(saves.acknowledge(id));
        assert!(!bundle.exists());
        assert_eq!(std::fs::read(&path).unwrap(), witness.expected_bytes());
        assert!(matches!(
            request(&worker, Command::OpenFieldRenamed { witness }),
            Ok(Response::Opened { .. })
        ));
        let Response::Data { columns, .. } = request(
            &worker,
            Command::Page {
                table: "Table1".into(),
                offset: 0,
            },
        )
        .unwrap() else {
            panic!("expected fresh renamed native columns");
        };
        assert_eq!(columns[1].name, new);
        assert_eq!(columns[1].native_number, Some(1));
        let Response::Data { columns, .. } = request(
            &worker,
            Command::Query {
                options: Default::default(),
                sql: format!("SELECT [{new}] FROM Table1"),
            },
        )
        .unwrap() else {
            panic!("expected explicitly executed query output");
        };
        assert!(columns.iter().all(|column| column.native_number.is_none()));
    }
}

#[test]
fn unchanged_review_consumes_to_read_only_witness_without_save_id_or_journal() {
    let before = raw::source(raw::SourceCase::Number);
    let (directory, path, worker, revision) = opened(&before);
    let (facts, capability) = review(&worker, revision, "Score", 1, "Score").unwrap();
    assert!(!facts.changed);
    let saves = SaveWorker::new(|| {});
    let bundle = directory.path().join("unchanged.journal");
    let refused = saves
        .start_field_rename(capability, bundle.clone())
        .unwrap_err();
    assert_eq!(refused.kind, SaveStartFailure::InvalidState);
    assert!(saves.poll().is_none());
    let witness = match refused.into_reviewed().unwrap().into_unchanged_reopen() {
        Ok(witness) => witness,
        Err(_) => panic!("current genuine unchanged review should consume to readonly witness"),
    };
    assert_eq!(witness.expected_bytes(), before);
    assert!(!witness.changed());
    assert!(!bundle.exists());
    assert!(matches!(
        request(&worker, Command::OpenFieldRenamed { witness }),
        Ok(Response::Opened { .. })
    ));
    assert_eq!(std::fs::read(path).unwrap(), before);
}

#[test]
fn stale_revision_exact_native_target_and_consuming_owner_refuse_without_writes() {
    let before = raw::source(raw::SourceCase::Number);
    let (directory, path, worker, revision) = opened(&before);
    assert!(matches!(
        review(&worker, revision + 1, "Score", 1, "Total"),
        Err(Error::StaleDocument)
    ));
    for (name, number) in [("score", 1), ("Score", 0), ("Score", 99)] {
        assert!(matches!(
            review(&worker, revision, name, number, "Total"),
            Err(Error::StaleFieldRenameTarget)
        ));
    }
    let (_, capability) = review(&worker, revision, "Score", 1, "Total").unwrap();
    let (_, unchanged) = review(&worker, revision, "Score", 1, "Score").unwrap();
    request(&worker, open(&path, "")).unwrap();
    assert!(unchanged.into_unchanged_reopen().is_err());
    let saves = SaveWorker::new(|| {});
    let bundle = directory.path().join("stale.journal");
    let error = saves
        .start_field_rename(capability, bundle.clone())
        .unwrap_err();
    assert_eq!(error.kind, SaveStartFailure::StaleDocument);
    assert!(error.into_reviewed().is_some());
    assert!(!bundle.exists());
    assert_eq!(std::fs::read(path).unwrap(), before);
}

#[test]
fn acknowledged_rename_requires_complete_after_witness_and_cancelled_read_preserves_file() {
    let before = raw::source(raw::SourceCase::Number);
    let (directory, path, worker, revision) = opened(&before);
    let (_, capability) = review(&worker, revision, "Score", 1, "Total").unwrap();
    let witness = capability.reopen_witness();
    let saves = SaveWorker::new(|| {});
    let id = saves
        .start_field_rename(capability, directory.path().join("actual.journal"))
        .unwrap();
    assert!(matches!(
        terminal(&saves).outcome,
        SaveOutcome::Saved { .. }
    ));
    assert!(saves.acknowledge(id));
    assert!(matches!(
        crate::document::Document::open_field_renamed(&witness, revision + 1, &|| true),
        Err(Error::Cancelled)
    ));
    let mut changed = std::fs::read(&path).unwrap();
    *changed.last_mut().unwrap() ^= 1;
    std::fs::write(&path, &changed).unwrap();
    assert!(matches!(
        request(&worker, Command::OpenFieldRenamed { witness }),
        Err(Error::FieldRenamedSourceChanged)
    ));
    assert_eq!(std::fs::read(path).unwrap(), changed);
}

#[test]
fn changed_source_after_unlocked_review_retains_release_owner_without_journal() {
    let before = raw::source(raw::SourceCase::Number);
    let (directory, path, worker, revision) = opened(&before);
    let (_, capability) = review(&worker, revision, "Score", 1, "Total").unwrap();
    let mut changed = before;
    changed[7 * 4096 + 3000] ^= 1;
    std::fs::write(&path, &changed).unwrap();
    let saves = SaveWorker::new(|| {});
    let bundle = directory.path().join("refused.journal");
    let id = saves
        .start_field_rename(capability, bundle.clone())
        .unwrap();
    let SaveOutcome::Failed(failure) = terminal(&saves).outcome else {
        panic!("stale bytes must not gain save authority");
    };
    assert_eq!(failure.kind, SaveFailureKind::SourceChanged);
    assert!(failure.can_release && !failure.can_recover);
    assert!(!bundle.exists());
    assert!(!saves.acknowledge(id));
    let release_id = saves.release(id).unwrap();
    assert!(matches!(
        terminal(&saves).outcome,
        SaveOutcome::Released { .. }
    ));
    assert!(saves.acknowledge(release_id));
    assert_eq!(std::fs::read(path).unwrap(), changed);
}

#[test]
fn cancellation_before_review_does_not_issue_a_capability_or_change_bytes() {
    let before = raw::source(raw::SourceCase::Number);
    let (_directory, path, _worker, revision) = opened(&before);
    let (mut document, _) =
        crate::document::Document::open(&path, &mut String::new(), revision, &|| false).unwrap();
    assert!(matches!(
        document.review_field_rename(revision, "Table1", "Score", 1, "Total", &|| true),
        Err(Error::Cancelled)
    ));
    assert_eq!(std::fs::read(path).unwrap(), before);
}

mod fitting;

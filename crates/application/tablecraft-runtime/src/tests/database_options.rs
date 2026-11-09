//! Explicit option review/save ownership, independent facts and guarded reopen.
mod field_history;
mod inspection;
use super::{open, request, stored::opened, terminal};
use crate::{
    Command, DatabaseOptionsReview, Error, Response, ReviewedDatabaseOptions,
    SaveDatabaseOptionsChange, SaveFailureKind, SaveOutcome, SaveStartFailure, SaveWorker,
};
use tablecraft_ace::{FieldHistoryDatabaseOptionsError, FieldHistoryOptionsIntentError};
use tablecraft_testkit::{
    fixtures::access::{canonical_creator_rename, canonical_database_options},
    support::access::retained_target::assert_retained_target_observation,
};

fn review(
    worker: &crate::Worker,
    revision: u64,
) -> (DatabaseOptionsReview, ReviewedDatabaseOptions) {
    let Response::DatabaseOptionsReviewed { review, capability } = request(
        worker,
        Command::ReviewDisableNameAutoCorrect {
            document_revision: revision,
        },
    )
    .unwrap() else {
        panic!("expected an explicit source-bound database-option review");
    };
    assert_eq!(review.document_revision, revision);
    assert_eq!(
        review.change,
        SaveDatabaseOptionsChange::NameAutoCorrectDisabled
    );
    (review, capability)
}

#[test]
fn explicit_off_saves_exact_images_and_reopens_before_separate_rename_review() {
    let originals = canonical_creator_rename::originals();
    for case in [0, 3, 6] {
        let before = &originals[case];
        let expected = canonical_database_options::explicit_off(before);
        let (directory, path, worker, revision) = opened(before);
        let (_, capability) = review(&worker, revision);
        let (_, second) = review(&worker, revision);
        let witness = capability.reopen_witness();
        assert_eq!(witness.source_path(), path);
        assert_eq!(witness.document_revision(), revision);
        assert_eq!(capability.source_path(), path);
        tablecraft_access::editing::offline::OfflineWriteSession::open(
            &path,
            crate::controls::SCOPE,
            &mut crate::controls::budget(),
            &|| false,
        )
        .unwrap()
        .release()
        .unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), *before);
        let saves = SaveWorker::new(|| {});
        let bundle = directory.path().join("options.journal");
        let id = saves
            .start_disable_name_auto_correct(capability, bundle.clone())
            .unwrap();
        let report = terminal(&saves);
        let SaveOutcome::Saved { receipt } = report.outcome else {
            panic!("expected completed explicit option save: {report:?}");
        };
        assert_eq!(
            receipt.database_options_change,
            Some(SaveDatabaseOptionsChange::NameAutoCorrectDisabled)
        );
        assert_eq!((receipt.matched_rows, receipt.changed_rows), (None, None));
        assert_eq!(receipt.schema_change, None);
        assert_eq!(receipt.query_change, None);
        assert_eq!(saves.poll(), Some(report));
        let busy = saves
            .start_disable_name_auto_correct(second, bundle.clone())
            .unwrap_err();
        assert_eq!(busy.kind, SaveStartFailure::Busy);
        assert!(busy.into_reviewed().is_some());
        assert!(saves.acknowledge(id));
        assert!(!bundle.exists());
        assert_eq!(std::fs::read(&path).unwrap(), expected);
        let Response::Opened {
            document_revision, ..
        } = request(&worker, Command::OpenDatabaseOptionsSaved { witness }).unwrap()
        else {
            panic!("expected a guarded read-only reopen");
        };
        assert!(matches!(
            request(
                &worker,
                Command::ReviewTableRename {
                    document_revision,
                    table: "Items".into(),
                    new_name: "Inventory".into(),
                }
            ),
            Ok(Response::TableRenameReviewed { .. })
        ));
        // Reviewing rename is separate; explicit OFF alone never renamed rows.
        assert_eq!(std::fs::read(&path).unwrap(), expected);
    }
}

#[test]
fn stale_option_source_retains_actual_target_without_creating_journal() {
    let before = canonical_creator_rename::originals().remove(0);
    let (directory, path, worker, revision) = opened(&before);
    let (_, capability) = review(&worker, revision);
    let mut changed = before;
    *changed.last_mut().unwrap() ^= 1;
    std::fs::write(&path, &changed).unwrap();
    let saves = SaveWorker::new(|| {});
    let bundle = directory.path().join("options.journal");
    let id = saves
        .start_disable_name_auto_correct(capability, bundle.clone())
        .unwrap();
    let SaveOutcome::Failed(failure) = terminal(&saves).outcome else {
        panic!("expected stale complete-source refusal");
    };
    assert_eq!(failure.kind, SaveFailureKind::SourceChanged);
    assert!(failure.can_release);
    assert!(!failure.can_recover);
    assert!(!saves.acknowledge(id));
    assert!(!bundle.exists());
    assert_retained_target_observation(&path, &changed);
    let release = saves.release(id).unwrap();
    assert!(matches!(
        terminal(&saves).outcome,
        SaveOutcome::Released { .. }
    ));
    assert!(saves.acknowledge(release));
    assert_eq!(std::fs::read(&path).unwrap(), changed);
}

#[test]
fn document_replacement_invalidates_option_review_and_returns_unconsumed_capability() {
    let before = canonical_creator_rename::originals().remove(0);
    let (directory, path, worker, revision) = opened(&before);
    let (_, capability) = review(&worker, revision);
    request(&worker, open(&path, "")).unwrap();
    let saves = SaveWorker::new(|| {});
    let bundle = directory.path().join("options.journal");
    let error = saves
        .start_disable_name_auto_correct(capability, bundle.clone())
        .unwrap_err();
    assert_eq!(error.kind, SaveStartFailure::StaleDocument);
    assert!(error.into_reviewed().is_some());
    assert!(!bundle.exists());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert!(matches!(
        request(
            &worker,
            Command::ReviewDisableNameAutoCorrect {
                document_revision: revision
            }
        ),
        Err(Error::StaleDocument)
    ));
}

#[test]
fn acknowledged_option_witness_refuses_changed_content_without_replaying_settings() {
    let before = canonical_creator_rename::originals().remove(0);
    let (directory, path, worker, revision) = opened(&before);
    let (_, capability) = review(&worker, revision);
    let witness = capability.reopen_witness();
    let saves = SaveWorker::new(|| {});
    let id = saves
        .start_disable_name_auto_correct(capability, directory.path().join("options.journal"))
        .unwrap();
    assert!(matches!(
        terminal(&saves).outcome,
        SaveOutcome::Saved { .. }
    ));
    assert!(saves.acknowledge(id));
    let mut changed = canonical_database_options::explicit_off(&before);
    *changed.last_mut().unwrap() = 1;
    std::fs::write(&path, &changed).unwrap();
    assert!(matches!(
        request(&worker, Command::OpenDatabaseOptionsSaved { witness }),
        Err(Error::DatabaseOptionsSourceChanged)
    ));
    assert_eq!(std::fs::read(&path).unwrap(), changed);
}

#[test]
fn cancelled_reviews_and_already_off_sources_refuse_without_writes() {
    let before =
        canonical_database_options::explicit_off(&canonical_creator_rename::originals().remove(0));
    let (_directory, path, worker, revision) = opened(&before);
    // Actual external column ownership selects FieldHistory before proof.
    // Its nine-role/already-OFF source refusal must not masquerade as a failed
    // creator attempt, nor trigger a retry through that frozen constructor.
    let result = request(
        &worker,
        Command::ReviewDisableNameAutoCorrect {
            document_revision: revision,
        },
    );
    assert!(
        matches!(
            result,
            Err(Error::FieldHistoryDatabaseOptions(
                FieldHistoryOptionsIntentError::Options(FieldHistoryDatabaseOptionsError::Source)
            ))
        ),
        "expected the selected field-history source refusal: {:?}",
        result.as_ref().err()
    );
    assert_eq!(std::fs::read(&path).unwrap(), before);

    let original = canonical_creator_rename::originals().remove(0);
    let (_directory, path, _worker, revision) = opened(&original);
    let (mut document, _) =
        crate::document::Document::open(&path, &mut String::new(), revision, &|| false).unwrap();
    assert!(matches!(
        document.review_disable_name_auto_correct(revision, &|| true),
        Err(Error::Cancelled)
    ));
    assert_eq!(std::fs::read(&path).unwrap(), original);
}

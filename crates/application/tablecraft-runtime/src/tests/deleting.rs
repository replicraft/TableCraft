//! Normal row deletion through the public review/save capability boundary.
use super::{open, request, stored::opened, terminal};
use crate::{
    Command, DeleteReview, DeleteTarget, EditError, Error, Response, ReviewedDelete,
    SaveFailureKind, SaveOutcome, SaveStartFailure, SaveWorker, Worker,
};
use tablecraft_testkit::fixtures::access::{canonical_creation, canonical_native17_creation};

fn source() -> Vec<u8> {
    canonical_native17_creation::source(
        canonical_creation::source(canonical_creation::Case::Extrema).bytes,
    )
}

fn review(worker: &Worker, revision: u64) -> (DeleteReview, ReviewedDelete) {
    let Response::DeleteReviewed { review, capability } = request(
        worker,
        Command::ReviewDelete {
            target: DeleteTarget {
                document_revision: revision,
                table: "Items".into(),
                parent_ordinal: 1,
            },
        },
    )
    .unwrap() else {
        panic!("expected a normal native deletion review");
    };
    assert_eq!(review.deleted_rows, 1);
    assert_eq!(
        review
            .columns
            .iter()
            .map(|column| column.name.as_str())
            .collect::<Vec<_>>(),
        ["Id", "Score"]
    );
    assert_eq!(
        review
            .row
            .iter()
            .map(|cell| cell.text.as_str())
            .collect::<Vec<_>>(),
        ["2", "2147483647"]
    );
    (review, capability)
}

#[test]
fn reviewed_second_row_deletion_commits_once_and_reopens_remaining_membership() {
    let original = source();
    let (directory, path, worker, revision) = opened(&original);
    let (_, capability) = review(&worker, revision);
    assert_eq!(capability.refresh_ordinal(), 0);
    assert_eq!(std::fs::read(&path).unwrap(), original);

    // Confirmation retains no native lease.
    tablecraft_access::editing::offline::OfflineWriteSession::open(
        &path,
        crate::controls::SCOPE,
        &mut crate::controls::budget(),
        &|| false,
    )
    .unwrap()
    .release()
    .unwrap();

    let saves = SaveWorker::new(|| {});
    let id = saves
        .start_delete(capability, directory.path().join("delete.journal"))
        .unwrap();
    let report = terminal(&saves);
    assert_eq!(report.id, id);
    let SaveOutcome::Saved { receipt } = report.outcome else {
        panic!("normal deletion must publish its completed receipt");
    };
    assert_eq!(receipt.matched_rows, Some(1));
    assert_eq!(receipt.changed_rows, Some(1));
    assert!(saves.acknowledge(id));

    request(&worker, open(&path, "")).unwrap();
    let Response::Data { rows, .. } = request(
        &worker,
        Command::Page {
            table: "Items".into(),
            offset: 0,
        },
    )
    .unwrap() else {
        panic!("native deletion must reopen through the ordinary reader");
    };
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0][0].text, "1");
    assert_eq!(rows[0][1].text, "-2147483648");
    assert!(!directory.path().join("delete.journal").exists());
}

#[test]
fn deletion_source_drift_retains_owner_until_release_without_inventing_outcome() {
    let original = source();
    let (directory, path, worker, revision) = opened(&original);
    let (_, capability) = review(&worker, revision);
    let mut changed = original.clone();
    *changed.last_mut().unwrap() ^= 1;
    std::fs::write(&path, &changed).unwrap();

    let saves = SaveWorker::new(|| {});
    let bundle = directory.path().join("delete.journal");
    let id = saves.start_delete(capability, bundle.clone()).unwrap();
    let SaveOutcome::Failed(failure) = terminal(&saves).outcome else {
        panic!("changed source must fail before deletion");
    };
    assert_eq!(failure.kind, SaveFailureKind::SourceChanged);
    assert!(failure.can_release);
    assert!(!failure.can_recover);
    assert!(!saves.acknowledge(id));
    assert!(!bundle.exists());

    let released = saves.release(id).unwrap();
    assert!(matches!(
        terminal(&saves).outcome,
        SaveOutcome::Released {
            current_outcome: None,
            last_completed: None
        }
    ));
    assert!(saves.acknowledge(released));
    assert_eq!(std::fs::read(&path).unwrap(), changed);
}

#[test]
fn deletion_capability_is_returned_on_busy_and_invalidated_by_reopen() {
    let original = source();
    let (directory, path, worker, revision) = opened(&original);
    let (_, first) = review(&worker, revision);
    let (_, second) = review(&worker, revision);
    let saves = SaveWorker::new(|| {});
    let id = saves
        .start_delete(first, directory.path().join("first.journal"))
        .unwrap();
    let error = saves
        .start_delete(second, directory.path().join("second.journal"))
        .unwrap_err();
    assert_eq!(error.kind, SaveStartFailure::Busy);
    let second = error.into_reviewed().unwrap();
    assert_eq!(terminal(&saves).id, id);
    assert!(saves.acknowledge(id));

    request(&worker, open(&path, "")).unwrap();
    let error = saves
        .start_delete(second, directory.path().join("stale.journal"))
        .unwrap_err();
    assert_eq!(error.kind, SaveStartFailure::StaleDocument);
    assert!(error.into_reviewed().is_some());
    assert!(!directory.path().join("stale.journal").exists());
}

#[test]
fn deletion_review_rejects_wrong_revision_and_missing_row_before_effects() {
    let original = source();
    let (_directory, path, worker, revision) = opened(&original);
    for (document_revision, parent_ordinal) in [(revision + 1, 1), (revision, 2)] {
        let result = request(
            &worker,
            Command::ReviewDelete {
                target: DeleteTarget {
                    document_revision,
                    table: "Items".into(),
                    parent_ordinal,
                },
            },
        );
        assert!(matches!(
            result,
            Err(Error::StaleDocument | Error::Edit(EditError::TargetMissing))
        ));
    }
    assert_eq!(std::fs::read(&path).unwrap(), original);
}

#[test]
fn deletion_refresh_position_uses_complete_remaining_membership() {
    for (case, ordinal, expected) in [
        (canonical_creation::Case::One, 0, 0),
        (canonical_creation::Case::Boundary, 1, 1),
        (canonical_creation::Case::Boundary, 63, 62),
    ] {
        let original = canonical_native17_creation::source(canonical_creation::source(case).bytes);
        let (_directory, _path, worker, revision) = opened(&original);
        let Response::DeleteReviewed { capability, .. } = request(
            &worker,
            Command::ReviewDelete {
                target: DeleteTarget {
                    document_revision: revision,
                    table: "Items".into(),
                    parent_ordinal: ordinal,
                },
            },
        )
        .unwrap() else {
            panic!("expected source-bound deletion review");
        };
        assert_eq!(capability.refresh_ordinal(), expected);
    }
}

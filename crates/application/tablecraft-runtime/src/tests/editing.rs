//! Real public-worker draft/review/save acceptance on independent native bytes.
mod genuine_jet4;

use super::{open, request, stored::opened, terminal};
use crate::{
    Command, EditInput, EditTarget, Response, ReviewedEdit, SaveOutcome, SaveStartFailure,
    SaveWorker,
};
use tablecraft_testkit::fixtures::access::mutation_fixture;
use tablecraft_testkit::support::access::retained_target::assert_retained_target_observation;

fn review(worker: &crate::Worker, revision: u64, text: &str) -> ReviewedEdit {
    let Response::EditDraft(draft) = request(
        worker,
        Command::BeginEdit {
            target: EditTarget {
                document_revision: revision,
                table: "Items".into(),
                column: "Score".into(),
                parent_ordinal: 1,
            },
        },
    )
    .unwrap() else {
        panic!("expected edit draft");
    };
    assert_eq!(draft.original.text, "20");
    assert_eq!(draft.initial.text, "20");
    let Response::EditReviewed { review, capability } = request(
        worker,
        Command::ReviewEdit {
            draft_id: draft.id,
            input: EditInput {
                text: text.into(),
                is_null: false,
            },
        },
    )
    .unwrap() else {
        panic!("expected ordinary native review");
    };
    assert_eq!(review.before.text, "20");
    assert_eq!(review.after.text, text);
    assert_eq!(review.changed_rows, 1);
    capability
}

#[test]
fn unlocked_review_commits_once_and_keeps_its_receipt_until_acknowledged() {
    let original = mutation_fixture::database();
    let (directory, path, worker, revision) = opened(&original);
    let capability = review(&worker, revision, "42");
    assert_eq!(capability.source_path(), path);
    assert_eq!(std::fs::read(&path).unwrap(), original);

    // A review holds no offline lease while the user considers the change.
    let mut budget = crate::controls::budget();
    tablecraft_access::editing::offline::OfflineWriteSession::open(
        &path,
        crate::controls::SCOPE,
        &mut budget,
        &|| false,
    )
    .unwrap()
    .release()
    .unwrap();

    let second = review(&worker, revision, "43");
    let saves = SaveWorker::new(|| {});
    let bundle = directory.path().join("save.journal");
    let id = saves.start(capability, bundle.clone()).unwrap();
    let rejected = saves
        .start(second, directory.path().join("second.journal"))
        .unwrap_err();
    assert_eq!(rejected.kind, SaveStartFailure::Busy);
    assert!(rejected.into_reviewed().is_some());
    worker
        .request(Command::Query {
            sql: "SELECT 1".into(),
        })
        .expect("fresh test worker identity");
    worker.cancel();
    let report = terminal(&saves);
    assert_eq!(report.id, id);
    let SaveOutcome::Saved { receipt } = report.outcome else {
        panic!("expected acknowledged native save: {report:?}");
    };
    assert_eq!(receipt.changed_rows, Some(1));
    assert_eq!(receipt.matched_rows, Some(1));
    assert_eq!(saves.poll(), Some(report));
    assert!(saves.is_busy());
    assert!(!saves.cancel(id));
    assert!(saves.acknowledge(id));
    assert!(!saves.is_busy());
    assert!(!bundle.exists());

    request(&worker, open(&path, "")).unwrap();
    let Response::Data { rows, .. } = request(
        &worker,
        Command::Page {
            table: "Items".into(),
            offset: 0,
        },
    )
    .unwrap() else {
        panic!("expected fresh native page");
    };
    assert_eq!(rows[1][1].text, "42");
    assert_eq!(rows[0][1].text, "10");
}

#[test]
fn unrelated_source_change_refuses_before_journal_and_retains_lease_until_release() {
    let original = mutation_fixture::database();
    let (directory, path, worker, revision) = opened(&original);
    let capability = review(&worker, revision, "42");
    let mut changed = original;
    *changed.last_mut().unwrap() ^= 1;
    std::fs::write(&path, &changed).unwrap();
    let saves = SaveWorker::new(|| {});
    let bundle = directory.path().join("stale.journal");
    let id = saves.start(capability, bundle.clone()).unwrap();
    let report = terminal(&saves);
    let SaveOutcome::Failed(failure) = report.outcome else {
        panic!("expected stale source refusal");
    };
    assert_eq!(failure.kind, crate::SaveFailureKind::SourceChanged);
    assert!(failure.can_release);
    assert!(!failure.can_recover);
    assert!(!saves.acknowledge(id));
    assert_retained_target_observation(&path, &changed);
    assert!(!bundle.exists());

    let release = saves.release(id).unwrap();
    assert_ne!(release, id);
    let report = terminal(&saves);
    assert!(matches!(
        report.outcome,
        SaveOutcome::Released {
            current_outcome: None,
            last_completed: None
        }
    ));
    assert!(saves.acknowledge(release));
    assert_eq!(std::fs::read(&path).unwrap(), changed);
}

#[test]
fn reopening_invalidates_an_old_capability_and_input_does_not_become_sql() {
    let original = mutation_fixture::database();
    let (directory, path, worker, revision) = opened(&original);
    let Response::EditDraft(draft) = request(
        &worker,
        Command::BeginEdit {
            target: EditTarget {
                document_revision: revision,
                table: "Items".into(),
                column: "Score".into(),
                parent_ordinal: 1,
            },
        },
    )
    .unwrap() else {
        panic!("expected draft");
    };
    assert!(
        request(
            &worker,
            Command::ReviewEdit {
                draft_id: draft.id,
                input: EditInput {
                    text: "1; DELETE FROM Items".into(),
                    is_null: false
                },
            }
        )
        .is_err()
    );
    let capability = review(&worker, revision, "42");
    request(&worker, open(&path, "")).unwrap();
    let saves = SaveWorker::new(|| {});
    let failure = saves
        .start(capability, directory.path().join("old.journal"))
        .unwrap_err();
    assert_eq!(failure.kind, SaveStartFailure::StaleDocument);
    assert!(failure.into_reviewed().is_some());
    assert!(!saves.is_busy());
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[test]
fn journal_collision_retains_diagnostics_until_explicit_resource_release() {
    let original = mutation_fixture::database();
    let (directory, path, worker, revision) = opened(&original);
    let capability = review(&worker, revision, "42");
    let saves = SaveWorker::new(|| {});
    let bundle = directory.path().join("occupied.journal");
    std::fs::create_dir(&bundle).unwrap();
    std::fs::write(bundle.join("sentinel"), b"existing namespace").unwrap();
    let id = saves.start(capability, bundle.clone()).unwrap();
    let report = terminal(&saves);
    let SaveOutcome::Failed(failure) = report.outcome else {
        panic!("expected retained setup failure");
    };
    assert_eq!(
        failure.state,
        Some(crate::SaveSessionState::IncompleteSetup)
    );
    assert!(!failure.can_recover);
    assert!(failure.can_release);
    assert!(!saves.acknowledge(id));
    assert!(saves.recover(id).is_err());
    assert_eq!(saves.poll(), Some(report));
    let release = saves.release(id).unwrap();
    assert_ne!(release, id);
    let released = terminal(&saves);
    assert_eq!(released.id, release);
    assert!(matches!(
        released.outcome,
        SaveOutcome::Released {
            current_outcome: None,
            last_completed: None
        }
    ));
    assert!(saves.acknowledge(release));
    assert!(!saves.is_busy());
    tablecraft_io::SharedFile::open(&path)
        .unwrap()
        .close()
        .unwrap();
    assert_eq!(std::fs::read(path).unwrap(), original);
    assert_eq!(
        std::fs::read(bundle.join("sentinel")).unwrap(),
        b"existing namespace"
    );
}

#[test]
fn reserved_journal_entries_block_reopening_without_following_or_cleaning_them() {
    let original = mutation_fixture::database();
    let (_directory, path, worker, _revision) = opened(&original);
    let reserved = crate::journal_path(&path);
    std::fs::write(&reserved, b"unresolved journal namespace").unwrap();
    assert!(matches!(
        request(&worker, open(&path, "")),
        Err(crate::Error::RetainedJournal)
    ));
    assert_eq!(
        std::fs::read(&reserved).unwrap(),
        b"unresolved journal namespace"
    );
    assert_eq!(std::fs::read(&path).unwrap(), original);
    std::fs::remove_file(&reserved).unwrap();

    #[cfg(unix)]
    {
        // A dangling symlink is still an unresolved reserved entry.
        std::os::unix::fs::symlink(path.with_file_name("absent-target"), &reserved).unwrap();
        assert!(matches!(
            request(&worker, open(&path, "")),
            Err(crate::Error::RetainedJournal)
        ));
        assert!(
            std::fs::symlink_metadata(&reserved)
                .unwrap()
                .file_type()
                .is_symlink()
        );
    }
}

#[test]
fn accepted_save_cancellation_never_discards_the_terminal_outcome() {
    let original = mutation_fixture::database();
    let (directory, _path, worker, revision) = opened(&original);
    let capability = review(&worker, revision, "42");
    let saves = SaveWorker::new(|| {});
    let id = saves
        .start(capability, directory.path().join("cancel.journal"))
        .unwrap();
    // This intentionally allows either side of the real filesystem completion
    // race. Cancellation is never evidence that a commit did not happen.
    saves.cancel(id);
    let report = terminal(&saves);
    assert_eq!(report.id, id);
    assert_eq!(saves.poll(), Some(report));
    match report.outcome {
        SaveOutcome::Saved { receipt } => {
            assert_eq!(receipt.known, crate::SaveKnownOutcome::Committed);
            assert!(saves.acknowledge(id));
        }
        SaveOutcome::Failed(failure) => {
            if failure.state.is_none() {
                assert!(saves.acknowledge(id));
            } else {
                assert!(!saves.acknowledge(id));
                assert!(saves.is_busy());
            }
        }
        SaveOutcome::Released { .. } => panic!("apply cannot manufacture a release-only result"),
    }
}

//! Unavailable save actors refuse capabilities before journal acceptance.
use super::*;
use crate::thread_start::{FailingSpawner, PausedSpawner};
use crate::{Command, EditInput, EditTarget, Response, Worker};
use std::time::{Duration, Instant};
use tablecraft_testkit::fixtures::access::mutation_fixture;

#[test]
fn failed_thread_start_returns_real_review_without_target_or_journal_work() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source.accdb");
    let before = mutation_fixture::database();
    std::fs::write(&source, &before).unwrap();
    let reader = Worker::new(|| {});
    let Response::Opened {
        document_revision, ..
    } = crate::tests::request(
        &reader,
        Command::Open {
            path: source.clone(),
            password: String::new(),
        },
    )
    .unwrap()
    else {
        panic!("expected original source");
    };
    let Response::EditDraft(draft) = crate::tests::request(
        &reader,
        Command::BeginEdit {
            target: EditTarget {
                document_revision,
                table: "Items".into(),
                column: "Score".into(),
                parent_ordinal: 1,
            },
        },
    )
    .unwrap() else {
        panic!("expected actual cell draft");
    };
    let Response::EditReviewed { capability, .. } = crate::tests::request(
        &reader,
        Command::ReviewEdit {
            draft_id: draft.id,
            input: EditInput {
                text: "42".into(),
                is_null: false,
            },
        },
    )
    .unwrap() else {
        panic!("expected actual consuming review");
    };
    let captured_path = capability
        .source_path()
        .as_os_str()
        .as_encoded_bytes()
        .as_ptr();
    let worker = SaveWorker::with_spawner(
        || panic!("unavailable actor must not repaint"),
        FailingSpawner,
    );
    let journal = directory.path().join("never-created.journal");

    let failure = worker.start(capability, journal.clone()).unwrap_err();
    assert_eq!(
        failure.kind,
        SaveStartFailure::ThreadStart(std::io::ErrorKind::WouldBlock)
    );
    let returned = failure.into_reviewed().unwrap();
    assert_eq!(returned.source_path(), source);
    assert_eq!(
        returned
            .source_path()
            .as_os_str()
            .as_encoded_bytes()
            .as_ptr(),
        captured_path
    );
    assert_eq!(std::fs::read(&source).unwrap(), before);
    assert!(!journal.exists());
    assert!(!worker.is_busy());
    assert!(worker.poll().is_none());
    assert_eq!(worker.shared.mailbox.lock().unwrap().next, 0);

    let failure = worker.start_reserved_recovery(source.clone()).unwrap_err();
    assert_eq!(
        failure.kind,
        SaveStartFailure::ThreadStart(std::io::ErrorKind::WouldBlock)
    );
    assert_eq!(failure.into_reviewed().unwrap(), source);
    assert_eq!(
        worker.recover(1).unwrap_err().kind,
        SaveStartFailure::ThreadStart(std::io::ErrorKind::WouldBlock)
    );
    assert_eq!(
        worker.release(1).unwrap_err().kind,
        SaveStartFailure::ThreadStart(std::io::ErrorKind::WouldBlock)
    );
    assert_eq!(std::fs::read(&source).unwrap(), before);

    // The returned capability remains usable. Dropping the replacement host
    // before its actor starts must still finish this accepted publication.
    let (spawner, release) = PausedSpawner::new();
    let (completed, observation) = std::sync::mpsc::channel();
    let replacement = SaveWorker::with_spawner(move || completed.send(()).unwrap(), spawner);
    replacement.start(returned, journal.clone()).unwrap();
    let owner = Arc::downgrade(&replacement.shared);
    drop(replacement);
    release.send(()).unwrap();
    observation.recv_timeout(Duration::from_secs(30)).unwrap();

    let deadline = Instant::now() + Duration::from_secs(5);
    while owner.upgrade().is_some() {
        assert!(
            Instant::now() < deadline,
            "completed save actor retained its owner"
        );
        std::thread::yield_now();
    }
    assert!(!journal.exists());
    crate::tests::request(
        &reader,
        Command::Open {
            path: source.clone(),
            password: String::new(),
        },
    )
    .unwrap();
    let Response::Data { rows, .. } = crate::tests::request(
        &reader,
        Command::Page {
            table: "Items".into(),
            offset: 0,
        },
    )
    .unwrap() else {
        panic!("expected the accepted saved image");
    };
    assert_eq!(rows[1][1].text, "42");
}

#[test]
fn shutdown_before_actor_wait_releases_its_captured_owner() {
    let (spawner, release) = PausedSpawner::new();
    let worker = SaveWorker::with_spawner(|| {}, spawner);
    let owner = Arc::downgrade(&worker.shared);
    drop(worker);
    release.send(()).unwrap();

    let deadline = Instant::now() + Duration::from_secs(5);
    while owner.upgrade().is_some() {
        assert!(
            Instant::now() < deadline,
            "idle save actor retained its owner"
        );
        std::thread::yield_now();
    }
}

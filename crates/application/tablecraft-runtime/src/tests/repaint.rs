//! A host notification failure must not destroy a worker or its real resource owner.
#[cfg(all(
    feature = "offline-edit",
    any(target_os = "macos", target_os = "linux", windows)
))]
use super::terminal;
use super::{open, query, request};
#[cfg(all(
    feature = "offline-edit",
    any(target_os = "macos", target_os = "linux", windows)
))]
use crate::{Command, EditInput, EditTarget, SaveFailureKind, SaveOutcome, SaveWorker};
use crate::{Response, Worker};
use std::{
    sync::atomic::{AtomicBool, Ordering},
    sync::mpsc,
    time::Duration,
};
use tablecraft_testkit::fixtures::access::mutation_fixture;

#[test]
fn panicked_read_notification_keeps_the_published_response_and_processes_the_next_query() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("notification.mdb");
    std::fs::write(&path, mutation_fixture::database()).unwrap();
    let (notified, received) = mpsc::sync_channel(1);
    let first = AtomicBool::new(true);
    let worker = Worker::new(move || {
        if first.swap(false, Ordering::AcqRel) {
            notified.send(()).unwrap();
            panic!("injected read notification failure");
        }
    });
    assert!(matches!(
        request(&worker, open(&path, "")).unwrap(),
        Response::Opened { .. }
    ));
    received.recv_timeout(Duration::from_secs(5)).unwrap();
    let Response::Data { rows, .. } = request(&worker, query("SELECT 1 AS Result")).unwrap() else {
        panic!("next operation must still execute after the callback fails");
    };
    assert_eq!(rows[0][0].text, "1");
}

#[cfg(all(
    feature = "offline-edit",
    any(target_os = "macos", target_os = "linux", windows)
))]
#[test]
fn panicked_save_notification_keeps_retained_failure_resources_until_explicit_release() {
    let original = mutation_fixture::database();
    let (directory, path, worker, revision) = super::stored::opened(&original);
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
        panic!("expected actual edit metadata");
    };
    let Response::EditReviewed { capability, .. } = request(
        &worker,
        Command::ReviewEdit {
            draft_id: draft.id,
            input: EditInput {
                text: "42".into(),
                is_null: false,
            },
        },
    )
    .unwrap() else {
        panic!("expected actual reviewed capability");
    };
    let mut changed = original;
    *changed.last_mut().unwrap() ^= 1;
    std::fs::write(&path, &changed).unwrap();
    let (notified, received) = mpsc::sync_channel(1);
    let first = AtomicBool::new(true);
    let saves = SaveWorker::new(move || {
        if first.swap(false, Ordering::AcqRel) {
            notified.send(()).unwrap();
            panic!("injected save notification failure");
        }
    });
    let id = saves
        .start(capability, directory.path().join("notification.journal"))
        .unwrap();
    let report = terminal(&saves);
    received.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(matches!(report.outcome, SaveOutcome::Failed(failure)
        if failure.kind == SaveFailureKind::SourceChanged && failure.can_release));
    assert!(!saves.acknowledge(id));
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

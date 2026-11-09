//! Startup refusals and durable shutdown preserve creation ownership.
use super::*;
use crate::thread_start::{FailingSpawner, PausedSpawner};
use std::{sync::mpsc, time::Duration};

#[cfg(any(unix, windows))]
#[test]
fn failed_thread_start_returns_the_original_destination_without_acceptance() {
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("not-created.accdb");
    let request = CreationRequest::blank(destination.clone());
    let original_buffer = request
        .destination()
        .as_os_str()
        .as_encoded_bytes()
        .as_ptr();
    let worker = CreationWorker::with_operation_and_spawner(
        || panic!("unavailable actor must not repaint"),
        |_, _| panic!("unavailable actor must not create a file"),
        FailingSpawner,
    );

    let failure = worker.start(request).unwrap_err();
    assert_eq!(
        failure.kind,
        CreationStartFailure::ThreadStart(std::io::ErrorKind::WouldBlock)
    );
    let request = failure.into_request();
    assert_eq!(request.destination(), destination);
    assert_eq!(
        request
            .destination()
            .as_os_str()
            .as_encoded_bytes()
            .as_ptr(),
        original_buffer
    );
    assert!(!destination.exists());
    assert!(!worker.is_busy());
    assert!(worker.poll().is_none());
    assert_eq!(worker.shared.mailbox.lock().unwrap().next, 0);
}

#[cfg(any(unix, windows))]
#[test]
fn shutdown_before_actor_wait_releases_its_captured_owner() {
    let (spawner, release) = PausedSpawner::new();
    let worker = CreationWorker::with_operation_and_spawner(|| {}, service::blank, spawner);
    let owner = Arc::downgrade(&worker.shared);
    drop(worker);
    release.send(()).unwrap();

    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while owner.upgrade().is_some() {
        assert!(
            std::time::Instant::now() < deadline,
            "idle creation actor retained its owner"
        );
        std::thread::yield_now();
    }
}

#[cfg(any(unix, windows))]
#[test]
fn shutdown_does_not_skip_an_already_accepted_creation() {
    let (spawner, release) = PausedSpawner::new();
    let (completed, observation) = mpsc::channel();
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("accepted.accdb");
    let worker = CreationWorker::with_operation_and_spawner(
        move || completed.send(()).unwrap(),
        service::blank,
        spawner,
    );
    worker
        .start(CreationRequest::blank(destination.clone()))
        .unwrap();
    let owner = Arc::downgrade(&worker.shared);
    drop(worker);
    release.send(()).unwrap();

    observation.recv_timeout(Duration::from_secs(30)).unwrap();
    assert!(destination.exists());
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while owner.upgrade().is_some() {
        assert!(
            std::time::Instant::now() < deadline,
            "completed creation actor retained its owner"
        );
        std::thread::yield_now();
    }
}

use super::*;
#[cfg(any(unix, windows))]
use crate::{Command, Response, Worker};
#[cfg(any(unix, windows))]
use std::{sync::mpsc, time::Duration};
#[cfg(any(unix, windows))]
use tablecraft_io::publication::PublicationGuarantee;

#[cfg(any(unix, windows))]
fn notified_worker() -> (CreationWorker, mpsc::Receiver<()>) {
    let (sender, receiver) = mpsc::channel();
    let worker = CreationWorker::new(move || {
        sender.send(()).unwrap();
    });
    (worker, receiver)
}

#[cfg(any(unix, windows))]
fn terminal(worker: &CreationWorker, receiver: &mpsc::Receiver<()>) -> Arc<CreationReport> {
    receiver.recv_timeout(Duration::from_secs(30)).unwrap();
    worker.poll().expect("completion precedes notification")
}

#[cfg(any(unix, windows))]
#[test]
fn fresh_blank_receipt_is_retained_and_reopens_through_normal_reads() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("blank.accdb");
    let (worker, notifications) = notified_worker();
    let id = worker.start(CreationRequest::blank(path.clone())).unwrap();
    let report = terminal(&worker, &notifications);
    assert_eq!(report.id, id);
    assert_eq!(report.destination(), path);
    let CreationOutcome::Created(receipt) = report.outcome() else {
        panic!("expected acknowledged fresh publication: {report:?}");
    };
    assert_eq!(receipt.table_count(), 0);
    assert_eq!(receipt.column_count(), 0);
    assert_eq!(receipt.row_count(), 0);
    assert_eq!(receipt.namespace_synced(), cfg!(unix));
    let expected_guarantee = if cfg!(windows) {
        PublicationGuarantee::NtfsWriteThroughRename
    } else {
        PublicationGuarantee::CapturedParentBarrier
    };
    assert_eq!(receipt.guarantee(), expected_guarantee);
    assert!(Arc::ptr_eq(&report, &worker.poll().unwrap()));
    assert!(worker.is_busy());
    assert!(!worker.cancel(id));
    assert!(!worker.acknowledge(id + 1));

    let other = directory.path().join("other.accdb");
    let rejected = worker
        .start(CreationRequest::blank(other.clone()))
        .unwrap_err();
    assert_eq!(rejected.kind, CreationStartFailure::Busy);
    assert_eq!(rejected.into_request().destination(), other);
    assert!(!other.exists());

    let bytes = std::fs::read(&path).unwrap();
    let reader = Worker::new(|| {});
    let Response::Opened { tables, .. } = crate::tests::request(
        &reader,
        Command::OpenCreated {
            report: Arc::clone(&report),
        },
    )
    .unwrap() else {
        panic!("expected fresh normal snapshot");
    };
    assert!(!tables.is_empty());
    assert!(tables.iter().all(|table| table.system));
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    std::fs::write(&path, b"unrelated changed contents").unwrap();
    assert!(matches!(
        crate::tests::request(
            &reader,
            Command::OpenCreated {
                report: Arc::clone(&report)
            }
        ),
        Err(crate::Error::CreatedSourceChanged)
    ));
    assert!(matches!(
        crate::tests::request(
            &reader,
            Command::Page {
                table: "MSysObjects".to_owned(),
                offset: 0
            }
        ),
        Err(crate::Error::NoDatabase)
    ));
    assert!(matches!(report.outcome(), CreationOutcome::Created(_)));
    assert_eq!(std::fs::read(&path).unwrap(), b"unrelated changed contents");
    assert!(worker.acknowledge(id));
    assert!(!worker.is_busy());
    assert!(worker.poll().is_none());
    assert!(!worker.acknowledge(id));
    assert!(matches!(report.outcome(), CreationOutcome::Created(_)));
}

#[cfg(any(unix, windows))]
#[test]
fn active_creation_cannot_be_replaced_and_cancel_keeps_a_terminal_receipt() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("cancelled.accdb");
    let other = directory.path().join("replacement.accdb");
    let entered = Arc::new(std::sync::Barrier::new(2));
    let release = Arc::new(std::sync::Barrier::new(2));
    let (sender, notifications) = mpsc::channel();
    let task_entered = Arc::clone(&entered);
    let task_release = Arc::clone(&release);
    let worker = CreationWorker::with_operation(
        move || sender.send(()).unwrap(),
        move |destination, cancelled| {
            task_entered.wait();
            task_release.wait();
            service::blank(destination, cancelled)
        },
    );
    let id = worker.start(CreationRequest::blank(path.clone())).unwrap();
    entered.wait();
    let rejected = worker
        .start(CreationRequest::blank(other.clone()))
        .unwrap_err();
    assert_eq!(rejected.kind, CreationStartFailure::Busy);
    assert_eq!(rejected.into_request().destination(), other);
    assert!(!worker.cancel(id + 1));
    assert!(!worker.acknowledge(id));
    assert!(worker.cancel(id));
    release.wait();

    let report = terminal(&worker, &notifications);
    let CreationOutcome::Failed(failure) = report.outcome() else {
        panic!("expected cancelled creation");
    };
    assert!(failure.is_cancelled());
    assert_eq!(failure.phase, Some(super::super::CreationPhase::Capture));
    assert_eq!(
        failure.publication,
        super::super::CreationPublication::NotPublished {
            temporary_may_remain: false,
        }
    );
    let reader = Worker::new(|| {});
    assert!(matches!(
        crate::tests::request(
            &reader,
            Command::OpenCreated {
                report: Arc::clone(&report)
            }
        ),
        Err(crate::Error::NoCreatedReceipt)
    ));
    assert!(!path.exists());
    assert!(!other.exists());
    assert!(Arc::ptr_eq(&report, &worker.poll().unwrap()));
    assert!(worker.acknowledge(id));
}

#[cfg(any(unix, windows))]
#[test]
fn existing_destination_is_preserved_and_requires_acknowledgement() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("occupied.accdb");
    let original = b"an existing destination must never be replaced";
    std::fs::write(&path, original).unwrap();
    let (worker, notifications) = notified_worker();
    let id = worker.start(CreationRequest::blank(path.clone())).unwrap();
    let report = terminal(&worker, &notifications);
    let CreationOutcome::Failed(failure) = report.outcome() else {
        panic!("existing destination must refuse");
    };
    assert_eq!(
        failure.publication,
        super::super::CreationPublication::NotPublished {
            temporary_may_remain: false,
        }
    );
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert!(worker.is_busy());
    assert!(!worker.cancel(id));
    assert!(worker.acknowledge(id));
}

#[cfg(any(unix, windows))]
#[test]
fn a_panicked_operation_retains_uncertain_visibility_without_retry() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("unknown.accdb");
    let (sender, notifications) = mpsc::channel();
    let worker = CreationWorker::with_operation(
        move || sender.send(()).unwrap(),
        |_, _| panic!("injected operation failure"),
    );
    let id = worker.start(CreationRequest::blank(path.clone())).unwrap();
    let report = terminal(&worker, &notifications);
    let CreationOutcome::Failed(failure) = report.outcome() else {
        panic!("expected retained panic outcome");
    };
    assert!(failure.worker_panicked());
    assert_eq!(failure.phase, None);
    assert_eq!(
        failure.publication,
        super::super::CreationPublication::VisibilityUncertain
    );
    assert!(Arc::ptr_eq(&report, &worker.poll().unwrap()));
    assert!(!worker.cancel(id));
    assert!(worker.acknowledge(id));
    assert!(!path.exists());
}

#[cfg(not(any(unix, windows)))]
#[test]
fn unsupported_platform_returns_the_unconsumed_request() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("unsupported.accdb");
    let worker = CreationWorker::new(|| {});
    let rejected = worker
        .start(CreationRequest::blank(path.clone()))
        .unwrap_err();
    assert_eq!(rejected.kind, CreationStartFailure::UnsupportedPlatform);
    assert_eq!(rejected.into_request().destination(), path);
    assert!(!path.exists());
    assert!(!worker.is_busy());
}

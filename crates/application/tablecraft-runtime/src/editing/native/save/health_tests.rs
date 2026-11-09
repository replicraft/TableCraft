//! Actual actor teardown, distinct from native publication and rollback evidence.
use super::*;
use crate::{
    controls::{SCOPE, budget},
    thread_start::PausedSpawner,
};
use std::time::{Duration, Instant};
use tablecraft_testkit::fixtures::access::mutation_fixture;

pub(super) struct PanicTask {
    released: Arc<AtomicBool>,
    session: Option<OfflineWriteSession>,
    reviewed: Option<ReviewedEdit>,
}

impl Drop for PanicTask {
    fn drop(&mut self) {
        drop(self.session.take());
        drop(self.reviewed.take());
        self.released.store(true, Ordering::Release);
    }
}

fn panic_task(task: PanicTask, _bundle: PathBuf) -> Task {
    Task::Panic(task)
}

fn terminal(worker: &SaveWorker) -> SaveReport {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(report) = worker.poll() {
            return report;
        }
        assert!(
            Instant::now() < deadline,
            "actual actor exit must publish uncertainty"
        );
        std::thread::yield_now();
    }
}

#[test]
fn actor_teardown_invalidates_source_before_repeatable_uncertainty_and_ack_never_resurrects() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source.accdb");
    let before = mutation_fixture::database();
    std::fs::write(&source, &before).unwrap();
    let released = Arc::new(AtomicBool::new(false));
    let (_reader, reviewed) = super::test_support::review_source(&source);
    let validity = Arc::clone(&reviewed.state.valid);
    let second = super::test_support::review(&_reader, reviewed.target().document_revision);
    let session = OfflineWriteSession::open(&source, SCOPE, &mut budget(), &|| false).unwrap();
    let worker = SaveWorker::new(|| {});
    let id = worker
        .start_reviewed(
            PanicTask {
                released: Arc::clone(&released),
                session: Some(session),
                reviewed: Some(reviewed),
            },
            directory.path().join("unused.journal"),
            Some(Arc::clone(&validity)),
            panic_task,
        )
        .unwrap();
    let report = terminal(&worker);
    assert!(released.load(Ordering::Acquire));
    assert!(!validity.load(Ordering::Acquire));
    let replacement = SaveWorker::new(|| {});
    let stale = replacement
        .start(second, directory.path().join("stale.journal"))
        .unwrap_err();
    assert_eq!(stale.kind, SaveStartFailure::StaleDocument);
    assert!(stale.into_reviewed().is_some());
    assert!(!directory.path().join("stale.journal").exists());
    assert_eq!(
        report,
        SaveReport {
            id,
            operation: SaveOperation::Apply,
            outcome: SaveOutcome::Uncertain {
                last_known_outcome: None,
                last_completed: None
            },
        }
    );
    assert!(!report.stopped_before_target_writes());
    assert_eq!(worker.poll(), Some(report));
    assert!(worker.is_busy());
    assert_eq!(std::fs::read(&source).unwrap(), before);
    let reopened = OfflineWriteSession::open(&source, SCOPE, &mut budget(), &|| false).unwrap();
    reopened.release().unwrap();
    assert_eq!(
        worker.recover(id).unwrap_err().kind,
        SaveStartFailure::WorkerStopped
    );
    assert_eq!(
        worker.release(id).unwrap_err().kind,
        SaveStartFailure::WorkerStopped
    );
    let rejected = worker.start_reserved_recovery(source.clone()).unwrap_err();
    assert_eq!(rejected.kind, SaveStartFailure::WorkerStopped);
    assert_eq!(rejected.into_reviewed(), Some(source));
    assert!(worker.acknowledge(id));
    assert!(!worker.is_busy());
    assert!(worker.poll().is_none());
    assert!(!worker.acknowledge(id));
}

#[test]
fn uncertain_resolution_keeps_accepted_operation_and_historical_facts_without_grants() {
    for operation in [SaveOperation::Recover, SaveOperation::Release] {
        let worker = SaveWorker::new(|| {});
        let id = worker
            .start_reviewed_operation(
                PanicTask {
                    released: Arc::new(AtomicBool::new(false)),
                    session: None,
                    reviewed: None,
                },
                PathBuf::new(),
                None,
                panic_task,
                operation,
            )
            .unwrap();
        let report = terminal(&worker);
        assert_eq!(report.id, id);
        assert_eq!(report.operation, operation);
        assert!(matches!(report.outcome, SaveOutcome::Uncertain { .. }));
        assert!(!report.stopped_before_target_writes());
    }
}

#[test]
fn completed_retained_facts_lose_grants_after_actor_loss_but_resource_free_result_is_not_overwritten()
 {
    let (spawner, release) = PausedSpawner::new();
    let worker = SaveWorker::with_spawner(|| {}, spawner);
    let validity = Arc::new(AtomicBool::new(true));
    let historical = SaveFailure {
        kind: SaveFailureKind::InvalidState,
        phase: None,
        state: None,
        current_outcome: Some(crate::SaveKnownOutcome::OriginalVerified),
        last_completed: None,
        can_recover: true,
        can_release: true,
    };
    {
        let mut mailbox = worker.shared.mailbox.lock().unwrap();
        mailbox.next = 9;
        mailbox.retained = true;
        mailbox.validity = Some(Arc::clone(&validity));
        mailbox.report = Some(SaveReport {
            id: 9,
            operation: SaveOperation::Apply,
            outcome: SaveOutcome::Failed(historical),
        });
    }
    worker.shared.stopped.store(true, Ordering::Release);
    release.send(()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !worker.shared.disconnected() {
        assert!(
            Instant::now() < deadline,
            "actor must actually end before lost-owner classification"
        );
        std::thread::yield_now();
    }
    let report = terminal(&worker);
    assert_eq!(report.id, 9);
    assert!(!validity.load(Ordering::Acquire));
    assert_eq!(
        report.outcome,
        SaveOutcome::Uncertain {
            last_known_outcome: Some(crate::SaveKnownOutcome::OriginalVerified),
            last_completed: None,
        }
    );
    assert!(!report.stopped_before_target_writes());
    assert!(worker.acknowledge(9));
    assert!(worker.poll().is_none());

    // A genuine completed no-resource result remains exact, even after exit.
    let genuine = SaveReport {
        id: 10,
        operation: SaveOperation::Release,
        outcome: SaveOutcome::Released {
            current_outcome: Some(crate::SaveKnownOutcome::OriginalVerified),
            last_completed: None,
        },
    };
    {
        let mut mailbox = worker.shared.mailbox.lock().unwrap();
        mailbox.next = 10;
        mailbox.report = Some(genuine);
    }
    assert_eq!(worker.poll(), Some(genuine));
    assert!(worker.acknowledge(10));
    assert!(worker.poll().is_none());
}

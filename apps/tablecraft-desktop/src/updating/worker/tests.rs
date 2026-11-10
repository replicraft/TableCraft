use super::*;

#[test]
fn stale_events_do_not_finish_the_current_operation_and_progress_is_not_terminal() {
    let mut worker = UpdateWorker::new(|| {});
    worker.generation = 2;
    worker.busy = true;
    let events = worker.event_sender.as_ref().unwrap();
    events
        .send(Reply::Event {
            generation: 1,
            event: UpdateEvent::Current,
        })
        .unwrap();
    events
        .send(Reply::Event {
            generation: 2,
            event: UpdateEvent::Downloading(tablecraft_update::DownloadProgress {
                received: 10,
                total: 100,
            }),
        })
        .unwrap();
    assert!(matches!(
        worker.poll(),
        Some(Reply::Event {
            generation: 2,
            event: UpdateEvent::Downloading(_)
        })
    ));
    assert!(worker.busy);
    worker
        .event_sender
        .as_ref()
        .unwrap()
        .send(Reply::Event {
            generation: 2,
            event: UpdateEvent::Ready {
                version: "0.1.1".into(),
            },
        })
        .unwrap();
    assert!(matches!(
        worker.poll(),
        Some(Reply::Event {
            event: UpdateEvent::Ready { .. },
            ..
        })
    ));
    assert!(!worker.busy);
}

#[test]
fn cancel_cannot_interrupt_an_irreversible_installation() {
    let mut worker = UpdateWorker::new(|| {});
    worker.installing = true;
    worker.cancel();
    assert!(!worker.cancelled.load(Ordering::Acquire));
    worker.installing = false;
    worker.cancel();
    assert!(worker.cancelled.load(Ordering::Acquire));
}

#[test]
fn stale_native_errors_survive_a_closed_progress_channel_without_finishing_current_install() {
    use tablecraft_update::apply::ApplyError;

    let mut worker = UpdateWorker::new(|| {});
    worker.generation = 2;
    worker.busy = true;
    worker.installing = true;
    let (sender, receiver) = mpsc::sync_channel(1);
    drop(receiver);
    assert!(post(
        Reply::Failed {
            generation: 1,
            error: UpdateTaskError::Apply(ApplyError::Layout),
        },
        &sender,
        &worker.retained,
        &|| {},
    ));
    let Some(Reply::Retained {
        generation,
        outcome,
    }) = worker.poll()
    else {
        panic!("stale native resource owner was discarded");
    };
    assert_eq!(generation, 1);
    assert!(matches!(
        &*outcome,
        RetainedOutcome::ApplyFailed(ApplyError::Layout)
    ));
    assert!(worker.busy);
    assert!(worker.installing);
    assert!(
        worker.poll().is_none(),
        "native owner must be delivered exactly once"
    );
}

#[test]
fn queued_stale_apply_error_is_promoted_before_generation_filtering() {
    use tablecraft_update::apply::ApplyError;

    let mut worker = UpdateWorker::new(|| {});
    worker.generation = 4;
    worker.busy = true;
    worker
        .event_sender
        .as_ref()
        .unwrap()
        .send(Reply::Failed {
            generation: 3,
            error: UpdateTaskError::Apply(ApplyError::Layout),
        })
        .unwrap();
    assert!(matches!(
        worker.poll(),
        Some(Reply::Retained { generation: 3, .. })
    ));
    assert!(worker.busy);
}

#[test]
fn actor_disconnect_reports_once_clears_busy_and_refuses_automatic_retry() {
    for installing in [false, true] {
        let mut worker = UpdateWorker::new(|| {});
        worker.generation = 7;
        worker.busy = true;
        worker.installing = installing;
        drop(worker.event_sender.take());
        assert!(matches!(
            worker.poll(),
            Some(Reply::Stopped { generation: 7, installation_uncertain }) if installation_uncertain == installing
        ));
        assert!(!worker.busy);
        assert!(!worker.installing);
        assert!(
            worker.poll().is_none(),
            "disconnect must not produce an endless failure stream"
        );
        assert!(!worker.start(UpdateCommand::Check));
    }
}

#[test]
fn native_result_is_taken_before_disconnect_without_inventing_an_uncertain_install() {
    use tablecraft_update::apply::ApplyError;

    let mut worker = UpdateWorker::new(|| {});
    worker.generation = 5;
    worker.busy = true;
    worker.installing = true;
    assert!(post(
        Reply::Failed {
            generation: 5,
            error: UpdateTaskError::Apply(ApplyError::Layout)
        },
        worker.event_sender.as_ref().unwrap(),
        &worker.retained,
        &|| {},
    ));
    drop(worker.event_sender.take());
    assert!(matches!(
        worker.poll(),
        Some(Reply::Retained { generation: 5, .. })
    ));
    assert!(!worker.busy);
    assert!(!worker.installing);
    assert!(
        worker.poll().is_none(),
        "a received native terminal is not a missing outcome"
    );
}

#[test]
fn mailbox_retains_real_backup_owners_across_ui_cancellation_and_contention() {
    use std::{fs, path::PathBuf};

    struct BackupOwner {
        path: PathBuf,
        dropped: Arc<AtomicBool>,
    }

    impl Drop for BackupOwner {
        fn drop(&mut self) {
            self.dropped.store(true, Ordering::Release);
        }
    }

    enum Outcome {
        Applied(BackupOwner),
        Failed(BackupOwner),
    }

    fn fake_helper_result(root: &std::path::Path, name: &str) -> BackupOwner {
        let source = root.join(name);
        fs::write(&source, b"original executable bytes").unwrap();
        let backup = tempfile::Builder::new()
            .prefix("retained-backup-")
            .tempdir_in(root)
            .unwrap()
            .keep();
        let original = backup.join("original");
        fs::rename(source, &original).unwrap();
        BackupOwner {
            path: original,
            dropped: Arc::new(AtomicBool::new(false)),
        }
    }

    let root = tempfile::tempdir().unwrap();
    let mailbox = Mailbox::new();
    let applied = fake_helper_result(root.path(), "old-app");
    let applied_dropped = Arc::clone(&applied.dropped);
    let failed = fake_helper_result(root.path(), "old-app-failed");
    let failed_dropped = Arc::clone(&failed.dropped);
    mailbox.push(RetainedEntry {
        generation: 1,
        outcome: Arc::new(Outcome::Applied(applied)),
    });
    mailbox.push(RetainedEntry {
        generation: 2,
        outcome: Arc::new(Outcome::Failed(failed)),
    });

    let held = mailbox.pending.lock();
    assert!(
        matches!(mailbox.poll(), MailboxPoll::Contended),
        "UI polling must not wait on the producer's lock"
    );
    assert!(!applied_dropped.load(Ordering::Acquire));
    assert!(!failed_dropped.load(Ordering::Acquire));
    drop(held);

    let mut host_owners = Vec::new();
    for expected in [1, 2] {
        let MailboxPoll::Ready(entry) = mailbox.poll() else {
            panic!("retained owner not delivered");
        };
        assert_eq!(entry.generation, expected);
        let owner = match &*entry.outcome {
            Outcome::Applied(owner) | Outcome::Failed(owner) => owner,
        };
        assert_eq!(fs::read(&owner.path).unwrap(), b"original executable bytes");
        host_owners.push(entry.outcome);
    }
    assert!(matches!(mailbox.poll(), MailboxPoll::Empty));
    assert!(!applied_dropped.load(Ordering::Acquire));
    assert!(!failed_dropped.load(Ordering::Acquire));
    drop(host_owners);
    assert!(applied_dropped.load(Ordering::Acquire));
    assert!(failed_dropped.load(Ordering::Acquire));
}

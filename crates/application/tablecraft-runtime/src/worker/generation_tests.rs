//! Boundary identities remain distinct from zero and the cancellation sentinel.
use super::{Command, EXHAUSTED_GENERATION, ReadRequestExhausted, Worker};
use crate::{Error, thread_start::FailingSpawner};
use std::sync::atomic::Ordering;

fn command() -> Command {
    Command::Page {
        table: "inert".into(),
        offset: 0,
    }
}

#[test]
fn final_identity_survives_rejected_successors_until_explicit_cancellation() {
    let worker = Worker::start_with_spawner(|| {}, |_, _| Err(Error::NoDatabase), FailingSpawner);
    worker
        .shared
        .generation
        .store(EXHAUSTED_GENERATION - 2, Ordering::Release);

    let last = worker.request(command()).unwrap();
    assert_eq!(last, u64::MAX - 1);
    assert!(worker.publication_is_current(last));
    for _ in 0..2 {
        assert_eq!(worker.request(command()), Err(ReadRequestExhausted));
        assert!(worker.publication_is_current(last));
    }

    let (id, result) = worker.poll().unwrap();
    assert_eq!(id, last);
    assert!(matches!(result, Err(Error::ReadWorkerStopped)));
    assert!(worker.poll().is_none());

    worker.cancel();
    assert!(!worker.publication_is_current(last));
    assert!(!worker.publication_is_current(0));
    assert_eq!(worker.request(command()), Err(ReadRequestExhausted));
    worker.cancel();
    assert_eq!(worker.request(command()), Err(ReadRequestExhausted));
    assert!(worker.poll().is_none());
}

#[test]
fn concurrent_final_admission_issues_one_identity_without_wraparound() {
    let worker = Worker::start_with_spawner(|| {}, |_, _| Err(Error::NoDatabase), FailingSpawner);
    worker
        .shared
        .generation
        .store(EXHAUSTED_GENERATION - 2, Ordering::Release);

    let results = std::thread::scope(|scope| {
        let first = scope.spawn(|| worker.request(command()));
        let second = scope.spawn(|| worker.request(command()));
        [first.join().unwrap(), second.join().unwrap()]
    });
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(results.iter().filter(|result| result.is_err()).count(), 1);
    assert_eq!(results.into_iter().find_map(Result::ok), Some(u64::MAX - 1));
    worker.cancel();
    assert!(worker.poll().is_none());
    assert_eq!(worker.request(command()), Err(ReadRequestExhausted));
}

#[test]
fn rejected_live_successor_does_not_replace_the_last_queued_command() {
    let (spawner, release) = crate::thread_start::PausedSpawner::new();
    let (executed, execution) = std::sync::mpsc::sync_channel(1);
    let worker = Worker::start_with_spawner(
        || {},
        move |command, _| {
            let Command::Page { offset, .. } = command else {
                panic!("expected explicit page command");
            };
            executed.send(*offset).unwrap();
            Err(Error::NoDatabase)
        },
        spawner,
    );
    worker
        .shared
        .generation
        .store(u64::MAX - 2, Ordering::Release);

    let last = worker.request(command()).unwrap();
    assert_eq!(
        worker.request(Command::Page {
            table: "rejected".into(),
            offset: 99,
        }),
        Err(ReadRequestExhausted)
    );
    release.send(()).unwrap();
    assert_eq!(
        execution
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap(),
        0
    );

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        if let Some((id, result)) = worker.poll() {
            assert_eq!(id, last);
            assert!(matches!(result, Err(Error::NoDatabase)));
            break;
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::yield_now();
    }
    worker.cancel();
    assert!(!worker.publication_is_current(last));
    assert!(!worker.publication_is_current(u64::MAX));
}

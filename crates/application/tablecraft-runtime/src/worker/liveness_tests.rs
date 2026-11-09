//! A terminal actor failure cannot strand a newer readonly publication.
use super::{Command, Error, Worker};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc::sync_channel,
    },
    time::{Duration, Instant},
};

fn request() -> Command {
    Command::Page {
        table: "inert".into(),
        offset: 0,
    }
}

fn terminal(worker: &Worker, wanted: u64) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some((id, reply)) = worker.poll() {
            assert_eq!(id, wanted);
            assert!(matches!(reply, Err(Error::ReadWorkerStopped)));
            break;
        }
        assert!(
            Instant::now() < deadline,
            "dead read actor must publish a terminal failure"
        );
        std::thread::yield_now();
    }
    assert!(worker.poll().is_none());
}

struct Invalidates(Arc<AtomicBool>);
impl Drop for Invalidates {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

#[test]
fn executor_is_dropped_before_terminal_error_and_future_requests_never_reenter_it() {
    let invalidated = Arc::new(AtomicBool::new(false));
    let owner = Invalidates(Arc::clone(&invalidated));
    let entered = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&entered);
    let worker = Worker::test_worker(move |_, _| {
        let _owned = &owner;
        count.fetch_add(1, Ordering::AcqRel);
        panic!("injected read operation failure");
    });
    terminal(
        &worker,
        worker
            .request(request())
            .expect("fresh test worker identity"),
    );
    assert!(invalidated.load(Ordering::Acquire));
    terminal(
        &worker,
        worker
            .request(request())
            .expect("fresh test worker identity"),
    );
    assert_eq!(entered.load(Ordering::Acquire), 1);
}

#[test]
fn a_queued_successor_receives_terminal_failure_when_the_previous_executor_panics() {
    let (entered, entry) = sync_channel(1);
    let (release, released) = sync_channel(1);
    let worker = Worker::test_worker(move |_, _| {
        entered.send(()).unwrap();
        released.recv().unwrap();
        panic!("injected predecessor failure");
    });
    let old = worker
        .request(request())
        .expect("fresh test worker identity");
    entry.recv_timeout(Duration::from_secs(5)).unwrap();
    let latest = worker
        .request(request())
        .expect("fresh test worker identity");
    assert!(latest > old);
    release.send(()).unwrap();
    terminal(&worker, latest);
}

#[test]
fn cancellation_revokes_dead_publication_and_failure_requests_still_notify_the_host() {
    let repaints = Arc::new(AtomicUsize::new(0));
    let calls = Arc::clone(&repaints);
    let worker = Worker::start(
        move || {
            calls.fetch_add(1, Ordering::AcqRel);
        },
        |_, _| {
            panic!("injected read failure");
        },
    );
    terminal(
        &worker,
        worker
            .request(request())
            .expect("fresh test worker identity"),
    );
    let failed = worker
        .request(request())
        .expect("fresh test worker identity");
    worker.cancel();
    assert!(!worker.publication_is_current(failed));
    assert!(worker.poll().is_none());
    terminal(
        &worker,
        worker
            .request(request())
            .expect("fresh test worker identity"),
    );
    assert!(repaints.load(Ordering::Acquire) >= 2);
}

#[test]
fn dropping_a_read_worker_never_waits_for_its_mailbox_or_joins_the_actor() {
    let worker = Worker::test_worker(|_, _| Err(Error::NoDatabase));
    let shared = Arc::clone(&worker.shared);
    let held = shared.pending.lock().unwrap();
    let (done, completed) = sync_channel(1);
    let dropper = std::thread::spawn(move || {
        drop(worker);
        done.send(()).unwrap();
    });
    let result = completed.recv_timeout(Duration::from_secs(1));
    drop(held);
    dropper.join().unwrap();
    assert!(
        result.is_ok(),
        "Drop must not acquire the held mailbox or wait for actor exit"
    );
}

#[test]
fn an_idle_actor_releases_its_actual_captured_owner_after_nonblocking_drop() {
    let released = Arc::new(AtomicBool::new(false));
    let captured = Invalidates(Arc::clone(&released));
    let worker = Worker::test_worker(move |_, _| {
        let _owned = &captured;
        Err(Error::NoDatabase)
    });
    let shared = Arc::clone(&worker.shared);
    // Delay actor access while Stop+notify is issued. No further request or
    // notification is allowed to make teardown succeed accidentally.
    let held = shared.pending.lock().unwrap();
    let (done, completed) = sync_channel(1);
    let dropper = std::thread::spawn(move || {
        drop(worker);
        done.send(()).unwrap();
    });
    let nonblocking = completed.recv_timeout(Duration::from_secs(1));
    drop(held);
    dropper.join().unwrap();
    assert!(
        nonblocking.is_ok(),
        "stop must not wait for the held request mailbox"
    );
    let deadline = Instant::now() + Duration::from_secs(2);
    while !released.load(Ordering::Acquire) {
        assert!(
            Instant::now() < deadline,
            "idle read actor must release its captured owner"
        );
        std::thread::yield_now();
    }
}

#[test]
fn spawn_failure_refuses_every_read_without_executing_or_opening_a_document() {
    let calls = Arc::new(AtomicUsize::new(0));
    let executed = Arc::clone(&calls);
    let worker = Worker::start_with_spawner(
        || {},
        move |_, _| {
            executed.fetch_add(1, Ordering::AcqRel);
            Err(Error::NoDatabase)
        },
        crate::thread_start::FailingSpawner,
    );
    terminal(
        &worker,
        worker
            .request(request())
            .expect("fresh test worker identity"),
    );
    terminal(
        &worker,
        worker
            .request(request())
            .expect("fresh test worker identity"),
    );
    assert_eq!(calls.load(Ordering::Acquire), 0);
}

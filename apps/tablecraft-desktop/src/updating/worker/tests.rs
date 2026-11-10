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

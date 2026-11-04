use super::{Command, Error, Response, Worker, read::Document};
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_testkit::fixtures::access::mutation_fixture;

fn query(sql: &str) -> Command {
    Command::Query {
        sql: sql.to_owned(),
    }
}

fn poll(worker: &Worker) -> (u64, Result<Response, Error>) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(reply) = worker.poll() {
            return reply;
        }
        assert!(Instant::now() < deadline, "worker did not finish");
        std::thread::yield_now();
    }
}

#[test]
fn native_sample_pages_and_queries_use_real_services() {
    let raw = mutation_fixture::database();
    let mut work = ResourceBudget::new(128 * 1024 * 1024, 10_000, 1_000_000);
    let (mut document, _) = Document::from_bytes(raw, "", &mut work, &|| false).unwrap();
    let Response::Data {
        columns,
        rows,
        has_more,
        ..
    } = document.page("Items", 0, &|| false).unwrap()
    else {
        panic!("expected data page");
    };
    assert_eq!(columns, ["Id", "Score"]);
    assert_eq!(rows, [["1", "10"], ["2", "20"], ["3", "30"]]);
    assert!(!has_more);

    let Response::Data {
        columns,
        rows,
        has_more,
        ..
    } = document
        .query(
            "SELECT [Id], [Score] FROM [Items] WHERE [Score] > 10 ORDER BY [Id] DESC",
            &|| false,
        )
        .unwrap()
    else {
        panic!("expected buffered query result");
    };
    assert_eq!(columns, ["Id", "Score"]);
    assert_eq!(rows, [["3", "30"], ["2", "20"]]);
    assert!(!has_more);
    assert!(document.query("DELETE FROM [Items]", &|| false).is_err());
    assert!(document.page("Items", 0, &|| true).is_err());
}

#[test]
fn latest_request_cancels_running_work_and_replaces_pending_work() {
    let (started_tx, started_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let (seen_tx, seen_rx) = mpsc::sync_channel(2);
    let mut first = true;
    let worker = Worker::test_worker(move |command, cancelled| {
        let Command::Query { sql } = command else {
            panic!("expected query");
        };
        seen_tx.send(sql.clone()).unwrap();
        if first {
            first = false;
            started_tx.send(()).unwrap();
            release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            assert!(cancelled());
            return Err(Error::Cancelled);
        }
        assert!(!cancelled());
        Ok(Response::Data {
            table: sql.clone(),
            offset: 0,
            columns: Vec::new(),
            rows: Vec::new(),
            has_more: false,
        })
    });
    worker.request(query("running"));
    started_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    worker.request(query("superseded pending"));
    let latest = worker.request(query("latest"));
    release_tx.send(()).unwrap();
    let (id, response) = poll(&worker);
    assert_eq!(id, latest);
    let Response::Data { table, .. } = response.unwrap() else {
        panic!("expected data");
    };
    assert_eq!(table, "latest");
    assert_eq!(seen_rx.recv().unwrap(), "running");
    assert_eq!(seen_rx.recv().unwrap(), "latest");
    assert!(seen_rx.try_recv().is_err());
    assert!(worker.poll().is_none());
}

#[test]
fn explicit_cancel_discards_a_late_result() {
    let (started_tx, started_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let (observed_tx, observed_rx) = mpsc::sync_channel(1);
    let worker = Worker::test_worker(move |_, cancelled| {
        started_tx.send(()).unwrap();
        release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        observed_tx.send(cancelled()).unwrap();
        Ok(Response::Opened {
            title: String::new(),
            format: String::new(),
            tables: Vec::new(),
        })
    });
    worker.request(query("running"));
    started_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    worker.cancel();
    release_tx.send(()).unwrap();
    assert!(observed_rx.recv_timeout(Duration::from_secs(5)).unwrap());
    assert!(worker.poll().is_none());
}

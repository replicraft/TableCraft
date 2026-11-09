#[cfg(all(
    feature = "offline-edit",
    feature = "ace16-create",
    any(target_os = "macos", target_os = "linux", windows)
))]
mod add_column;
mod boolean;
mod captions;
mod contents;
#[cfg(all(
    feature = "offline-edit",
    feature = "ace16-create",
    any(target_os = "macos", target_os = "linux", windows)
))]
mod database_options;
#[cfg(all(
    feature = "offline-edit",
    any(target_os = "macos", target_os = "linux", windows)
))]
mod deleting;
#[cfg(all(
    feature = "offline-edit",
    any(target_os = "macos", target_os = "linux", windows)
))]
mod editing;
#[cfg(all(
    feature = "offline-edit",
    feature = "ace16-create",
    any(target_os = "macos", target_os = "linux", windows)
))]
mod field_addition;
#[cfg(all(
    feature = "offline-edit",
    feature = "ace16-create",
    any(target_os = "macos", target_os = "linux", windows)
))]
mod field_rename;
mod filtering;
mod find;
mod formatting;
#[cfg(all(
    feature = "offline-edit",
    any(target_os = "macos", target_os = "linux", windows)
))]
mod inserting;
#[cfg(all(
    feature = "offline-edit",
    feature = "ace16-create",
    any(target_os = "macos", target_os = "linux", windows)
))]
mod number_column;
mod query_context;
#[cfg(all(
    feature = "offline-edit",
    any(target_os = "macos", target_os = "linux", windows)
))]
mod query_delete;
mod query_design;
#[cfg(all(
    feature = "offline-edit",
    any(target_os = "macos", target_os = "linux", windows)
))]
mod query_expression;
#[cfg(all(
    feature = "offline-edit",
    any(target_os = "macos", target_os = "linux", windows)
))]
mod query_insert;
mod query_parameters;
#[cfg(all(
    feature = "offline-edit",
    any(target_os = "macos", target_os = "linux", windows)
))]
mod query_sql;
#[cfg(all(
    feature = "offline-edit",
    any(target_os = "macos", target_os = "linux", windows)
))]
mod query_update;
mod repaint;
#[cfg(all(
    feature = "offline-edit",
    any(target_os = "macos", target_os = "linux", windows)
))]
mod replacing;
#[cfg(all(
    feature = "offline-edit",
    any(target_os = "macos", target_os = "linux", windows)
))]
mod restart;
mod saved;
mod schema;
mod stored;
mod table_creation_availability;
#[cfg(all(
    feature = "offline-edit",
    any(target_os = "macos", target_os = "linux", windows)
))]
mod table_rename;
mod text_policy;
mod totals;

use super::{Command, Error, Response, SortSpec, SortTarget, Worker};
use std::{
    io::Cursor,
    path::{Path, PathBuf},
    sync::{
        Arc, Barrier, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
use tablecraft_ace::{EncryptionRewrite, InspectError, rewrite_encryption};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_testkit::fixtures::access::mutation_fixture;

fn query(sql: &str) -> Command {
    Command::Query {
        sql: sql.to_owned(),
    }
}

fn row_texts(rows: &[Vec<crate::Cell>]) -> Vec<Vec<&str>> {
    rows.iter()
        .map(|row| row.iter().map(|cell| cell.text.as_str()).collect())
        .collect()
}

#[cfg(all(
    feature = "offline-edit",
    any(target_os = "macos", target_os = "linux", windows)
))]
pub(crate) fn terminal(worker: &crate::SaveWorker) -> crate::SaveReport {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(report) = worker.poll() {
            return report;
        }
        assert!(
            Instant::now() < deadline,
            "save did not publish a terminal outcome"
        );
        std::thread::yield_now();
    }
}

fn poll(worker: &Worker) -> (u64, Result<Response, Error>) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(reply) = worker.poll() {
            return reply;
        }
        assert!(Instant::now() < deadline, "worker did not finish");
        std::thread::yield_now();
    }
}

fn open(path: &Path, password: &str) -> Command {
    Command::Open {
        path: path.to_owned(),
        password: password.to_owned(),
    }
}

pub(crate) fn request(worker: &Worker, command: Command) -> Result<Response, Error> {
    let expected = worker.request(command);
    let (actual, response) = poll(worker);
    assert_eq!(actual, expected);
    response
}

#[test]
fn native_sample_pages_and_queries_use_real_services() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("sample.mdb");
    let original = mutation_fixture::database();
    std::fs::write(&path, &original).unwrap();
    let worker = Worker::new(|| {});
    let Response::Opened {
        title,
        source_path,
        tables,
        ..
    } = request(&worker, open(&path, "")).unwrap()
    else {
        panic!("expected opened database");
    };
    assert_eq!(title, "sample.mdb");
    assert_eq!(source_path, std::path::absolute(&path).unwrap());
    assert!(
        tables
            .iter()
            .any(|table| table.name == "Items" && !table.system)
    );

    // Reads use the captured image after the original handle has been released.
    std::fs::write(&path, b"changed outside the read session").unwrap();
    let Response::Data {
        columns,
        rows,
        has_more,
        ..
    } = request(
        &worker,
        Command::Page {
            table: "Items".to_owned(),
            offset: 0,
        },
    )
    .unwrap()
    else {
        panic!("expected data page");
    };
    assert_eq!(
        columns
            .iter()
            .map(|column| column.name.as_str())
            .collect::<Vec<_>>(),
        ["Id", "Score"]
    );
    assert_eq!(row_texts(&rows), [["1", "10"], ["2", "20"], ["3", "30"]]);
    assert!(!has_more);

    let Response::Data {
        columns,
        rows,
        has_more,
        ..
    } = request(
        &worker,
        query("SELECT [Id], [Score] FROM [Items] WHERE [Score] > 10 ORDER BY [Id] DESC"),
    )
    .unwrap()
    else {
        panic!("expected buffered query result");
    };
    assert_eq!(
        columns
            .iter()
            .map(|column| column.name.as_str())
            .collect::<Vec<_>>(),
        ["Id", "Score"]
    );
    assert_eq!(row_texts(&rows), [["3", "30"], ["2", "20"]]);
    assert!(!has_more);
    assert!(request(&worker, query("DELETE FROM [Items]")).is_err());

    // A new failed open closes the old session; subsequent rows cannot quietly
    // come from the database previously shown under another path.
    let absent = directory.path().join("absent.accdb");
    assert!(matches!(
        request(&worker, open(&absent, "private credential")),
        Err(Error::Io(_))
    ));
    assert!(matches!(
        request(
            &worker,
            Command::Page {
                table: "Items".to_owned(),
                offset: 0,
            }
        ),
        Err(Error::NoDatabase)
    ));
    std::fs::write(&path, &original).unwrap();
    assert!(matches!(
        request(&worker, open(&path, "")),
        Ok(Response::Opened { .. })
    ));
    assert_eq!(std::fs::read(&path).unwrap(), original);
}

#[test]
fn encrypted_open_reports_password_errors_and_allows_same_worker_retry() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/examples/Access_Example_VBA.accdb");
    let original = std::fs::read(path).unwrap();
    // This integration derivative exercises application routing and retry. The
    // engine's independent crypto goldens own ciphertext correctness separately.
    let encrypted = rewrite_encryption(
        &mut Cursor::new(&original),
        EncryptionRewrite::Add {
            password: None,
            new_password: "public workflow password",
        },
        &mut ResourceBudget::new(512 * 1024 * 1024, 100_000, 10_000_000),
        &|| false,
    )
    .unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("encrypted.accdb");
    std::fs::write(&path, encrypted.as_bytes()).unwrap();
    let worker = Worker::new(|| {});
    for (password, expected) in [
        ("", InspectError::PasswordRequired),
        ("incorrect", InspectError::IncorrectPassword),
    ] {
        assert!(
            matches!(request(&worker, open(&path, password)), Err(Error::Native(error)) if error == expected)
        );
        assert!(matches!(
            request(&worker, query("SELECT 1")),
            Err(Error::NoDatabase)
        ));
    }
    let Response::Opened {
        tables,
        document_revision: _revision,
        table_creation_available,
        table_design_available,
        ..
    } = request(&worker, open(&path, "public workflow password")).unwrap()
    else {
        panic!("expected authenticated database");
    };
    assert!(!table_creation_available);
    assert!(!table_design_available);
    assert!(tables.iter().any(|table| table.name == "ROULETTE_TB"));
    #[cfg(feature = "offline-edit")]
    assert!(matches!(
        request(
            &worker,
            Command::BeginEdit {
                target: crate::EditTarget {
                    document_revision: _revision,
                    table: "ROULETTE_TB".into(),
                    column: "ID".into(),
                    parent_ordinal: 0,
                },
            }
        ),
        Err(Error::Edit(crate::EditError::ReadOnlySource))
    ));
    let Response::Data { rows, .. } = request(
        &worker,
        Command::Page {
            table: "ROULETTE_TB".to_owned(),
            offset: 0,
        },
    )
    .unwrap() else {
        panic!("expected decrypted native rows");
    };
    assert!(!rows.is_empty());
    assert_eq!(std::fs::read(&path).unwrap(), encrypted.as_bytes());
}

#[test]
fn invalid_path_error_is_discarded_when_a_new_open_supersedes_it() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("valid.mdb");
    std::fs::write(&path, mutation_fixture::database()).unwrap();
    let (published_tx, published_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let release_rx = Mutex::new(release_rx);
    let first = AtomicBool::new(true);
    let worker = Worker::new(move || {
        if first.swap(false, Ordering::AcqRel) {
            published_tx.send(()).unwrap();
            release_rx
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(5))
                .unwrap();
        }
    });
    worker.request(open(&directory.path().join("absent.accdb"), ""));
    published_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    let newest = worker.request(open(&path, ""));
    assert!(worker.poll().is_none());
    release_tx.send(()).unwrap();
    let (actual, response) = poll(&worker);
    assert_eq!(actual, newest);
    assert!(matches!(response, Ok(Response::Opened { title, .. }) if title == "valid.mdb"));
    assert!(worker.poll().is_none());
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
            source_revision: None,
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
            document_revision: 1,
            source_path: PathBuf::new(),
            table_creation_available: false,
            table_design_available: false,
            title: String::new(),
            format: String::new(),
            tables: Vec::new(),
            queries: Vec::new(),
        })
    });
    worker.request(query("running"));
    started_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    worker.cancel();
    release_tx.send(()).unwrap();
    assert!(observed_rx.recv_timeout(Duration::from_secs(5)).unwrap());
    assert!(worker.poll().is_none());
}

#[test]
fn concurrent_mailbox_requests_publish_the_highest_returned_generation() {
    let worker = Arc::new(Worker::test_worker(|command, _| {
        let Command::Query { sql } = command else {
            panic!("expected query");
        };
        Ok(Response::Data {
            source_revision: None,
            table: sql.clone(),
            offset: 0,
            columns: Vec::new(),
            rows: Vec::new(),
            has_more: false,
        })
    }));

    // Fast completions race mailbox replacement; concurrent producers race
    // generation issuance. Each round must still publish its highest ID exactly
    // once without requiring another request to release a stranded UI operation.
    for round in 0..16 {
        let start = Arc::new(Barrier::new(4));
        let mut producers = Vec::new();
        for producer in 0..4 {
            let worker = Arc::clone(&worker);
            let start = Arc::clone(&start);
            producers.push(std::thread::spawn(move || {
                start.wait();
                let mut last = None;
                for ordinal in 0..32 {
                    let label = format!("{round}:{producer}:{ordinal}");
                    let id = worker.request(query(&label));
                    last = Some((id, label));
                }
                last.unwrap()
            }));
        }
        let expected = producers
            .into_iter()
            .map(|producer| producer.join().unwrap())
            .max_by_key(|(id, _)| *id)
            .unwrap();
        let (id, response) = poll(&worker);
        assert_eq!(id, expected.0);
        let Response::Data { table, .. } = response.unwrap() else {
            panic!("expected completed newest operation");
        };
        assert_eq!(table, expected.1);
        assert!(worker.poll().is_none());
    }
}

#[test]
fn sorting_reads_the_complete_relation_and_retains_native_edit_identity() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("sorted.mdb");
    let records = (1..=105).map(|id| (id, id * 10)).collect::<Vec<_>>();
    std::fs::write(&path, mutation_fixture::with_rows(&records)).unwrap();
    let worker = Worker::new(|| {});
    let Response::Opened {
        document_revision, ..
    } = request(&worker, open(&path, "")).unwrap()
    else {
        panic!("expected document");
    };
    let Response::FilteredData(sorted) = request(
        &worker,
        Command::SortView {
            target: SortTarget {
                document_revision,
                table: "Items".into(),
                filter_id: None,
            },
            sort: Some(SortSpec {
                column: "Score".into(),
                descending: true,
            }),
            base: None,
            enabled: false,
        },
    )
    .unwrap() else {
        panic!("expected sorted result");
    };
    assert_eq!(sorted.target.document_revision, document_revision);
    assert_eq!(sorted.rows.len(), 100);
    assert_eq!(sorted.rows[0][1].text, "1050");
    assert_eq!(sorted.source_ordinals[0], 104);
    assert!(sorted.has_more);
    let Response::FilteredData(tail) = request(
        &worker,
        Command::FilterPage {
            handle: sorted.handle,
            offset: 100,
            enabled: false,
        },
    )
    .unwrap() else {
        panic!("native sorted tail");
    };
    assert_eq!(tail.rows.len(), 5);
    assert_eq!(tail.rows[4][1].text, "10");
    assert_eq!(tail.source_ordinals, [4, 3, 2, 1, 0]);
    assert!(!tail.has_more);
    assert!(matches!(
        request(
            &worker,
            Command::SortView {
                target: SortTarget {
                    document_revision: document_revision + 1,
                    table: "Items".into(),
                    filter_id: None,
                },
                sort: Some(SortSpec {
                    column: "Score".into(),
                    descending: false
                }),
                base: None,
                enabled: false,
            }
        ),
        Err(Error::StaleDocument)
    ));
    assert_eq!(
        std::fs::read(path).unwrap(),
        mutation_fixture::with_rows(&records)
    );
}

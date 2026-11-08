//! Original presence and captured clock input survive consuming SQL INSERT saves.
use super::{open, query_parameters::inspect, request, stored::opened, terminal};
use crate::{
    Command, Error, ParameterInput, Response, ReviewedQueryInsert, SaveFailureKind, SaveOutcome,
    SaveWorker,
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use tablecraft_core::values::ExtendedDateTime;
use tablecraft_expressions::{
    EvalError,
    functions::dates::clock::{Clock, ClockReading},
};
use tablecraft_testkit::{
    fixtures::access::canonical_creation_defaults as fixtures,
    support::access::retained_target::assert_retained_target_observation,
};

fn input(text: &str) -> ParameterInput {
    ParameterInput {
        name: "new_id".into(),
        kind: "int32".into(),
        text: text.into(),
        is_null: false,
    }
}

fn review(
    worker: &crate::Worker,
    revision: u64,
    sql: &str,
    parameters: Vec<ParameterInput>,
) -> (crate::QueryInsertReview, ReviewedQueryInsert) {
    let (target, _) = inspect(worker, revision, sql);
    let Response::QueryInsertReviewed { review, capability } =
        request(worker, Command::ReviewQueryInsert { target, parameters }).unwrap()
    else {
        panic!("expected reviewed SQL INSERT");
    };
    assert_eq!(review.document_revision, revision);
    assert_eq!(review.table, "Items");
    (review, capability)
}

fn save(capability: ReviewedQueryInsert, bundle: std::path::PathBuf) {
    let saves = SaveWorker::new(|| {});
    let id = saves.start_query_insert(capability, bundle).unwrap();
    let report = terminal(&saves);
    let SaveOutcome::Saved { receipt } = report.outcome else {
        panic!("expected native SQL INSERT save: {report:?}");
    };
    assert_eq!(receipt.matched_rows, None);
    assert_eq!(receipt.changed_rows, Some(1));
    assert!(saves.acknowledge(id));
}

#[test]
fn omitted_default_and_explicit_null_remain_distinct_through_native_save_and_reopen() {
    for supplied_null in [false, true] {
        let source = fixtures::source(fixtures::Case::Seven);
        let (expected, rows) = fixtures::inserted(&source, supplied_null).unwrap();
        let (directory, path, worker, revision) = opened(&source.bytes);
        let sql = if supplied_null {
            "PARAMETERS new_id LONG; INSERT INTO Items (Id,Score) VALUES (new_id,NULL)"
        } else {
            "PARAMETERS new_id LONG; INSERT INTO Items (Id) VALUES (new_id)"
        };
        let (review, capability) = review(
            &worker,
            revision,
            sql,
            vec![input(if supplied_null { "25" } else { "24" })],
        );
        assert_eq!(review.inserted_rows, 1);
        assert_eq!(capability.source_path(), path);
        assert_eq!(capability.table(), "Items");
        assert_eq!(capability.document_revision(), revision);
        assert!(
            std::fs::read(&path).unwrap() == source.bytes,
            "review modified database"
        );
        tablecraft_access::editing::offline::OfflineWriteSession::open(
            &path,
            crate::controls::SCOPE,
            &mut crate::controls::budget(),
            &|| false,
        )
        .unwrap()
        .release()
        .unwrap();
        save(capability, directory.path().join("insert.journal"));
        assert!(
            std::fs::read(&path).unwrap() == expected,
            "committed INSERT differs from independent image"
        );
        request(&worker, open(&path, "")).unwrap();
        let Response::Data {
            rows: displayed, ..
        } = request(
            &worker,
            Command::Page {
                table: "Items".into(),
                offset: 0,
            },
        )
        .unwrap()
        else {
            panic!("expected reopened inserted rows");
        };
        assert_eq!(displayed.len(), rows.len());
        assert_eq!(
            displayed[1][0].text,
            if supplied_null { "25" } else { "24" }
        );
        assert!(
            matches!(displayed[1][1].kind, crate::CellKind::Scalar { is_null, .. } if is_null == supplied_null)
        );
        if !supplied_null {
            assert_eq!(displayed[1][1].text, "7");
        }
    }
}

struct OnceClock {
    calls: AtomicUsize,
}

impl Clock for OnceClock {
    fn read(&self) -> Result<ClockReading, EvalError> {
        if self.calls.fetch_add(1, Ordering::Relaxed) != 0 {
            return Err(EvalError::ClockUnavailable);
        }
        ClockReading::new(ExtendedDateTime::new(1899, 12, 30, 12, 0, 0, 0).unwrap())
    }
}

#[test]
fn needed_clock_is_sampled_once_at_review_and_never_during_save() {
    let original = fixtures::clock_source();
    let expected = fixtures::clock_inserted(&original, 24, 0.5_f64.to_bits());
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("clock.accdb");
    std::fs::write(&path, &original).unwrap();
    let clock = Arc::new(OnceClock {
        calls: AtomicUsize::new(0),
    });
    let worker = crate::Worker::with_query_clock(|| {}, clock.clone());
    let Response::Opened {
        document_revision, ..
    } = request(&worker, open(&path, "")).unwrap()
    else {
        panic!("expected document");
    };
    let sql = "PARAMETERS new_id LONG; INSERT INTO Items (Id) VALUES (new_id)";
    let (target, _) = inspect(&worker, document_revision, sql);
    assert_eq!(clock.calls.load(Ordering::Relaxed), 0);
    let Response::QueryInsertReviewed { capability, .. } = request(
        &worker,
        Command::ReviewQueryInsert {
            target,
            parameters: vec![input("24")],
        },
    )
    .unwrap() else {
        panic!("expected captured-clock INSERT");
    };
    assert_eq!(clock.calls.load(Ordering::Relaxed), 1);
    save(capability, directory.path().join("clock.journal"));
    assert_eq!(clock.calls.load(Ordering::Relaxed), 1);
    assert!(
        std::fs::read(path).unwrap() == expected,
        "retained clock INSERT differs from independent image"
    );
}

#[test]
fn stale_source_failure_retains_same_owner_until_explicit_release() {
    let source = fixtures::source(fixtures::Case::Seven);
    let (directory, path, worker, revision) = opened(&source.bytes);
    let (_, capability) = review(
        &worker,
        revision,
        "PARAMETERS new_id LONG; INSERT INTO Items (Id) VALUES (new_id)",
        vec![input("24")],
    );
    let mut changed = source.bytes;
    *changed.last_mut().unwrap() ^= 1;
    std::fs::write(&path, &changed).unwrap();
    let saves = SaveWorker::new(|| {});
    let bundle = directory.path().join("stale-insert.journal");
    let id = saves
        .start_query_insert(capability, bundle.clone())
        .unwrap();
    let SaveOutcome::Failed(failure) = terminal(&saves).outcome else {
        panic!("expected stale INSERT refusal");
    };
    assert_eq!(failure.kind, SaveFailureKind::SourceChanged);
    assert!(failure.can_release);
    assert!(!failure.can_recover);
    assert!(!saves.acknowledge(id));
    assert!(!bundle.exists());
    assert_retained_target_observation(&path, &changed);
    let released = saves.release(id).unwrap();
    assert!(matches!(
        terminal(&saves).outcome,
        SaveOutcome::Released {
            current_outcome: None,
            last_completed: None
        }
    ));
    assert!(saves.acknowledge(released));
    assert!(
        std::fs::read(path).unwrap() == changed,
        "refused INSERT modified source"
    );
}

#[test]
fn malformed_inputs_and_stale_inspection_refuse_before_data_or_clock() {
    let mut original = fixtures::clock_source();
    original[8 * 4096] = 0;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("unreadable.accdb");
    std::fs::write(&path, &original).unwrap();
    let clock = Arc::new(OnceClock {
        calls: AtomicUsize::new(0),
    });
    let worker = crate::Worker::with_query_clock(|| {}, clock.clone());
    let Response::Opened {
        document_revision, ..
    } = request(&worker, open(&path, "")).unwrap()
    else {
        panic!("expected document");
    };
    let (target, _) = inspect(
        &worker,
        document_revision,
        "PARAMETERS new_id LONG; INSERT INTO Items (Id) VALUES (new_id)",
    );
    for parameters in [
        vec![],
        vec![ParameterInput {
            kind: "text".into(),
            ..input("24")
        }],
        vec![input("24"), input("24")],
    ] {
        assert!(matches!(
            request(
                &worker,
                Command::ReviewQueryInsert {
                    target: target.clone(),
                    parameters
                }
            ),
            Err(Error::ParameterInput)
        ));
    }
    assert!(matches!(
        request(
            &worker,
            Command::ReviewQueryInsert {
                target: target.clone(),
                parameters: vec![input("DELETE FROM Items")]
            }
        ),
        Err(Error::InvalidScalarInput)
    ));
    assert_eq!(clock.calls.load(Ordering::Relaxed), 0);
    request(&worker, open(&path, "")).unwrap();
    assert!(matches!(
        request(
            &worker,
            Command::ReviewQueryInsert {
                target,
                parameters: vec![input("24")]
            }
        ),
        Err(Error::StaleQuery)
    ));
    assert_eq!(clock.calls.load(Ordering::Relaxed), 0);
    assert!(
        std::fs::read(path).unwrap() == original,
        "invalid INSERT input modified source"
    );
}

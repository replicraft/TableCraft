//! Supplied query time is evaluated only during the consuming make-table review.
use super::{assert_image, fields, populated};
use crate::tests::schema_sql::{inspect, review};
use crate::tests::{open, request, terminal};
use crate::{Command, Error, Response, SaveOutcome, SaveWorker, Worker};
use std::{fs, sync::Arc};
use tablecraft_core::values::ExtendedDateTime;
use tablecraft_expressions::functions::dates::clock::ClockReading;
use tablecraft_testkit::support::expressions::clock::CountingClock;

#[test]
fn make_table_date_context_is_lazy_source_bound_and_not_reexecuted_after_review() {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let before = populated::numbers(&seed, "Items", &[None, Some(2), Some(5), Some(9)]);
    let expected = populated::numbers(&before, "Archive", &[Some(2031), Some(2035)]);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("clock.accdb");
    fs::write(&path, &before).unwrap();
    let clock = Arc::new(CountingClock::new(
        ClockReading::new(ExtendedDateTime::new(2026, 11, 7, 13, 14, 15, 0).unwrap()).unwrap(),
    ));
    let worker = Worker::with_query_clock(|| {}, clock.clone());
    let Response::Opened {
        document_revision, ..
    } = request(&worker, open(&path, "")).unwrap()
    else {
        panic!("expected original document");
    };
    let sql = "SELECT CLng(Year(Date())+i.CopiedScore) AS CopiedScore INTO Archive FROM Items AS i WHERE Date()=#11/7/2026# AND i.CopiedScore>=5 ORDER BY i.CopiedScore";
    let stale = inspect(&worker, document_revision, sql);
    assert_eq!(clock.calls(), 0);
    let Response::Opened {
        document_revision, ..
    } = request(&worker, open(&path, "")).unwrap()
    else {
        panic!("expected fresh original document");
    };
    assert!(matches!(review(&worker, &stale), Err(Error::StaleQuery)));
    assert_eq!(clock.calls(), 0);
    let target = inspect(&worker, document_revision, sql);
    assert_eq!(clock.calls(), 0);
    let Response::TableCreationReviewed { capability } = review(&worker, &target).unwrap() else {
        panic!("expected complete time-filtered make-table review");
    };
    assert!(clock.calls() > 0);
    let reviewed_calls = clock.calls();
    let witness = capability.reopen_witness();
    assert_eq!(witness.inserted_rows(), Some(2));
    assert_image(witness.expected_bytes(), &expected);
    assert_image(&fs::read(&path).unwrap(), &before);
    let saves = SaveWorker::new(|| {});
    let bundle = crate::journal_path(&path);
    let id = saves
        .start_table_creation(capability, bundle.clone())
        .unwrap();
    let report = terminal(&saves);
    assert_eq!(report.id, id);
    assert!(matches!(report.outcome, SaveOutcome::Saved { .. }));
    assert!(saves.acknowledge(id));
    assert_eq!(clock.calls(), reviewed_calls);
    assert!(matches!(
        request(&worker, Command::OpenTableCreated { witness }),
        Ok(Response::Opened { .. })
    ));
    assert_eq!(clock.calls(), reviewed_calls);
    assert_image(&fs::read(&path).unwrap(), &expected);
    assert!(!bundle.exists());
}

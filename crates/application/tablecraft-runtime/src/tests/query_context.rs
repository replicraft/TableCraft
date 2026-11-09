//! Real scratch and saved execution share explicit, lazy host context.
use super::{open, poll, query, request};
use crate::{Cell, Command, Error, Response, SavedQueryTarget, Worker};
use std::{
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc,
    },
    time::Duration,
};
use tablecraft_core::values::{ExtendedDateTime, LegacyDateTime};
use tablecraft_display::{DateRepresentation, ScalarPresentation};
use tablecraft_expressions::{
    EvalError,
    functions::dates::clock::{Clock, ClockReading},
};
use tablecraft_query::QueryError;
use tablecraft_testkit::fixtures::access::{
    canonical_blank_creation, mutation_fixture,
    saved_queries::{Fragment, QUERY_ID, database_with_query_flags},
};

const WAIT: Duration = Duration::from_secs(5);
// Independent IEEE-754 goldens: 46,333 days since the Automation epoch and
// 47,655 / 86,400 for the time. These do not use the clock projection encoder.
const DATE_BITS: u64 = 0x40e6_9fa0_0000_0000;
const TIME_BITS: u64 = 0x3fe1_a666_6666_6666;
const NOW_BITS: u64 = 0x40e6_9fb1_a666_6666;

#[derive(Default)]
struct CountingClock {
    reads: AtomicUsize,
    unavailable: AtomicBool,
}

impl Clock for CountingClock {
    fn read(&self) -> Result<ClockReading, EvalError> {
        self.reads.fetch_add(1, Ordering::AcqRel);
        if self.unavailable.load(Ordering::Acquire) {
            return Err(EvalError::ClockUnavailable);
        }
        ClockReading::new(ExtendedDateTime::new(2026, 11, 7, 13, 14, 15, 0).unwrap())
    }
}

fn rows(response: Response) -> Vec<Vec<Cell>> {
    let Response::Data {
        rows,
        has_more,
        source_revision,
        ..
    } = response
    else {
        panic!("expected complete query data");
    };
    assert!(!has_more);
    assert!(source_revision.is_none());
    rows
}

fn assert_date(cell: &Cell, expected_bits: u64) {
    let Some(ScalarPresentation::DateTime {
        calendar,
        representation: DateRepresentation::LegacyBits(bits),
        ..
    }) = cell.presentation
    else {
        panic!("clock result must retain its exact legacy DATE carrier");
    };
    assert_eq!(bits, expected_bits);
    assert_eq!(
        calendar,
        LegacyDateTime::from_bits(expected_bits)
            .unwrap()
            .calendar_rounded_to_nanos()
            .unwrap()
    );
}

#[test]
fn explicit_clock_runs_on_existing_jet4_ace2007_and_ace2016_read_profiles() {
    let sample = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/examples/Access_Example_VBA.accdb");
    let sources = [
        ("Jet4", mutation_fixture::database()),
        ("Ace2007", std::fs::read(sample).unwrap()),
        ("Ace2016", canonical_blank_creation::source()),
    ];
    for (profile, original) in sources {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("context.accdb");
        std::fs::write(&path, &original).unwrap();
        let clock = Arc::new(CountingClock::default());
        let worker = Worker::with_query_clock(|| {}, clock.clone());
        let Response::Opened { format, .. } = request(&worker, open(&path, "")).unwrap() else {
            panic!("expected opened {profile} source");
        };
        assert!(
            format.contains(profile),
            "unexpected source profile: {format}"
        );
        assert_eq!(clock.reads.load(Ordering::Acquire), 0);

        let result =
            rows(request(&worker, query("SELECT Date(), Time(), Now(), Timer()")).unwrap());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].len(), 4);
        assert_date(&result[0][0], DATE_BITS);
        assert_date(&result[0][1], TIME_BITS);
        assert_date(&result[0][2], NOW_BITS);
        assert_eq!(result[0][3].text, "47655");
        assert_eq!(clock.reads.load(Ordering::Acquire), 4);
        assert_eq!(std::fs::read(path).unwrap(), original);
    }
}

#[test]
fn saved_inspection_and_failed_binding_are_inert_and_execution_reuses_scratch_context() {
    let original = database_with_query_flags(
        &[Fragment {
            object: Some(QUERY_ID),
            attribute: Some(6),
            order: Some(&[0, 0, 0, 1]),
            expression: Some("Now()"),
            ..Default::default()
        }],
        0,
    );
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("saved.accdb");
    std::fs::write(&path, &original).unwrap();
    let clock = Arc::new(CountingClock::default());
    let worker = Worker::with_query_clock(|| {}, clock.clone());
    let Response::Opened {
        document_revision,
        queries,
        ..
    } = request(&worker, open(&path, "")).unwrap()
    else {
        panic!("expected saved-query inventory");
    };
    let saved = queries.iter().find(|query| query.name == "Saved").unwrap();
    let target = SavedQueryTarget {
        document_revision,
        object_id: saved.object_id,
        name: saved.name.clone(),
    };
    assert!(matches!(
        request(&worker, Command::InspectSavedQuery { target: target.clone() }).unwrap(),
        Response::SavedQuery { parameters, .. } if parameters.is_empty()
    ));
    assert!(request(&worker, query("SELECT Now() FROM [Missing]")).is_err());
    assert_eq!(clock.reads.load(Ordering::Acquire), 0);

    let scratch = rows(request(&worker, query("SELECT Now()")).unwrap());
    let saved = rows(
        request(
            &worker,
            Command::RunSavedQuery {
                options: Default::default(),
                target,
                parameters: Vec::new(),
            },
        )
        .unwrap(),
    );
    assert_eq!(scratch[0][0].presentation, saved[0][0].presentation);
    assert_eq!(clock.reads.load(Ordering::Acquire), 2);
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[test]
fn random_state_resets_per_run_and_clock_failure_never_republishes_previous_results() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("session.mdb");
    std::fs::write(&path, mutation_fixture::database()).unwrap();
    let clock = Arc::new(CountingClock::default());
    let worker = Worker::with_query_clock(|| {}, clock.clone());
    request(&worker, open(&path, "")).unwrap();
    for _ in 0..2 {
        let result = rows(
            request(
                &worker,
                query("SELECT Rnd(0), Rnd(), Rnd(0), CurrentUser(), Error(5)"),
            )
            .unwrap(),
        );
        let values = &result[0];
        assert_eq!(
            values[0].text.parse::<f32>().unwrap().to_bits(),
            0.01953125_f32.to_bits()
        );
        assert_eq!(
            values[1].text.parse::<f32>().unwrap().to_bits(),
            0.7055475_f32.to_bits()
        );
        assert_eq!(values[1].text, values[2].text);
        assert_eq!(values[3].text, "Admin");
        assert_eq!(values[4].text, "Invalid procedure call or argument");
    }
    assert_eq!(clock.reads.load(Ordering::Acquire), 0);
    rows(request(&worker, query("SELECT Now()")).unwrap());
    clock.unavailable.store(true, Ordering::Release);
    assert!(matches!(
        request(&worker, query("SELECT Now()")),
        Err(Error::Query(QueryError::Evaluation(
            EvalError::ClockUnavailable
        )))
    ));
    assert!(worker.poll().is_none());
    clock.unavailable.store(false, Ordering::Release);
    let result = rows(request(&worker, query("SELECT Now()")).unwrap());
    assert_date(&result[0][0], NOW_BITS);
}

struct BlockingClock {
    entered: mpsc::SyncSender<()>,
    release: Mutex<mpsc::Receiver<()>>,
}

impl Clock for BlockingClock {
    fn read(&self) -> Result<ClockReading, EvalError> {
        self.entered.send(()).unwrap();
        self.release.lock().unwrap().recv_timeout(WAIT).unwrap();
        CountingClock::default().read()
    }
}

#[test]
fn superseded_real_clock_execution_cannot_publish_over_the_latest_command() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("cancel.mdb");
    std::fs::write(&path, mutation_fixture::database()).unwrap();
    let (entered, observed) = mpsc::sync_channel(1);
    let (release, released) = mpsc::sync_channel(1);
    let worker = Worker::with_query_clock(
        || {},
        Arc::new(BlockingClock {
            entered,
            release: Mutex::new(released),
        }),
    );
    request(&worker, open(&path, "")).unwrap();
    worker
        .request(query("SELECT Now()"))
        .expect("fresh test worker identity");
    observed.recv_timeout(WAIT).unwrap();
    let latest = worker
        .request(query("SELECT CurrentUser()"))
        .expect("fresh test worker identity");
    release.send(()).unwrap();
    let (actual, response) = poll(&worker);
    assert_eq!(actual, latest);
    let result = rows(response.unwrap());
    assert_eq!(result[0][0].text, "Admin");
    assert!(worker.poll().is_none());
}

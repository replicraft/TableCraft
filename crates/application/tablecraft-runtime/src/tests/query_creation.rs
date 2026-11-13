//! New QueryDef creation uses the actual read worker, save actor and guarded reopen.
use super::{request, terminal};
use crate::{
    Command, Error, QueryCreationOrigin, QueryCreationTarget, Response, SaveOutcome,
    SaveQueryChange, SaveWorker, SavedQueryTarget, Worker,
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use tablecraft_core::values::{ExtendedDateTime, LegacyDateTime};
use tablecraft_expressions::{
    EvalError,
    functions::dates::clock::{Clock, ClockReading},
};
use tablecraft_testkit::fixtures::access::saved_queries::creation as raw;

struct CountedClock(AtomicUsize);

impl Clock for CountedClock {
    fn read(&self) -> Result<ClockReading, EvalError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        ClockReading::new(ExtendedDateTime::new(2023, 3, 15, 12, 0, 0, 0).unwrap())
    }
}

fn captured_at() -> LegacyDateTime {
    LegacyDateTime::from_ole_days(45000.5).unwrap()
}

#[test]
fn authored_and_stored_sources_create_distinct_objects_without_replaying_clock_or_sql() {
    for origin in ["scratch", "authored-saved", "stored-copy"] {
        let saved_editor = origin != "scratch";
        let value = if origin == "stored-copy" { 42 } else { 99 };
        let expected = raw::source("Created", "Answer", value, captured_at());
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("queries.accdb");
        std::fs::write(&path, &expected.before).unwrap();
        let clock = Arc::new(CountedClock(AtomicUsize::new(0)));
        // NO_CLONE: The injected worker and the observer independently retain
        // the same clock counter to detect reads during save/reopen/Run.
        let worker = Worker::with_query_clock(|| {}, clock.clone());
        // NO_CLONE: Open consumes its path while the test independently owns
        // the source path for exact full-image and no-journal assertions.
        let Response::Opened {
            document_revision,
            queries,
            ..
        } = request(
            &worker,
            Command::Open {
                path: path.clone(),
                password: String::new(),
            },
        )
        .unwrap()
        else {
            panic!("expected real native document");
        };
        let source = if saved_editor {
            let saved = queries
                .into_iter()
                .find(|query| query.name == "Saved")
                .unwrap();
            Some(SavedQueryTarget {
                document_revision,
                object_id: saved.object_id,
                name: saved.name,
            })
        } else {
            None
        };
        let command = if origin == "stored-copy" {
            Command::ReviewStoredQueryCopy {
                target: source.unwrap(),
                destination: "Created".into(),
            }
        } else {
            Command::ReviewQueryCreation {
                target: QueryCreationTarget {
                    document_revision,
                    draft_revision: 7,
                    source,
                },
                sql: "SELECT 99 AS Answer;".into(),
                destination: "Created".into(),
            }
        };
        let (submitted_sql, submitted_name) = match &command {
            Command::ReviewQueryCreation { target, sql, .. } => (
                Some(sql.as_ptr() as usize),
                target
                    .source
                    .as_ref()
                    .map(|source| source.name.as_ptr() as usize),
            ),
            Command::ReviewStoredQueryCopy { target, .. } => {
                (None, Some(target.name.as_ptr() as usize))
            }
            _ => unreachable!("only the two explicit creation sources are issued"),
        };
        let Response::QueryCreationReviewed { review, capability } =
            request(&worker, command).unwrap()
        else {
            panic!("expected nominal new-query review");
        };
        assert_eq!(review.id, capability.id());
        assert_eq!(review.document_revision, document_revision);
        assert_eq!(review.native_id, expected.native_query_id);
        assert_eq!(review.value, value);
        assert_eq!(review.captured_at, captured_at());
        match review.origin {
            QueryCreationOrigin::Draft {
                draft_revision,
                sql,
                source,
            } => {
                assert_ne!(origin, "stored-copy");
                assert_eq!(draft_revision, 7);
                assert_eq!(sql, "SELECT 99 AS Answer;");
                assert_eq!(Some(sql.as_ptr() as usize), submitted_sql);
                assert_eq!(source.is_some(), saved_editor);
                assert_eq!(
                    source.as_ref().map(|source| source.name.as_ptr() as usize),
                    submitted_name,
                );
            }
            QueryCreationOrigin::Saved(source) => {
                assert_eq!(origin, "stored-copy");
                assert_eq!(Some(source.name.as_ptr() as usize), submitted_name);
                assert_eq!(source.name, "Saved");
                assert_eq!(source.document_revision, document_revision);
            }
        }
        assert_eq!(clock.0.load(Ordering::SeqCst), 1);
        assert_eq!(std::fs::read(&path).unwrap(), expected.before);
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
        let witness = capability.reopen_witness();
        let saves = SaveWorker::new(|| {});
        let id = saves
            .start_query_creation(capability, directory.path().join("creation.tcj"))
            .unwrap();
        let SaveOutcome::Saved { receipt } = terminal(&saves).outcome else {
            panic!("expected real query-created save");
        };
        assert_eq!(receipt.query_change, Some(SaveQueryChange::Created));
        assert_eq!(
            (
                receipt.matched_rows,
                receipt.changed_rows,
                receipt.schema_change
            ),
            (None, None, None)
        );
        assert!(
            SaveOutcome::Saved { receipt }
                .query_expression_reopen_facts()
                .is_err()
        );
        assert!(
            SaveOutcome::Saved { receipt }
                .query_creation_reopen_facts()
                .unwrap()
                .replacement
        );
        assert_eq!(std::fs::read(&path).unwrap(), expected.expected);
        assert_eq!(clock.0.load(Ordering::SeqCst), 1);
        assert!(saves.acknowledge(id));
        let Response::Opened {
            document_revision: fresh_revision,
            queries,
            ..
        } = request(&worker, Command::OpenQueryCreated { witness }).unwrap()
        else {
            panic!("expected exact new-image catalog publication");
        };
        assert_ne!(fresh_revision, document_revision);
        let created_value = if origin == "stored-copy" { "42" } else { "99" };
        for (name, value) in [("Created", created_value), ("Saved", "42")] {
            let query = queries.iter().find(|query| query.name == name).unwrap();
            // NO_CLONE: Each explicit Run owns its selected fresh query name;
            // the catalog list remains available for the other protected query.
            let target = SavedQueryTarget {
                document_revision: fresh_revision,
                object_id: query.object_id,
                name: query.name.clone(),
            };
            let Response::Data { rows, .. } = request(
                &worker,
                Command::RunSavedQuery {
                    target,
                    options: Default::default(),
                    parameters: Vec::new(),
                },
            )
            .unwrap() else {
                panic!("expected explicit saved Run");
            };
            assert_eq!(rows[0][0].text, value);
        }
        assert_eq!(clock.0.load(Ordering::SeqCst), 1);
    }
}

#[test]
fn stale_authored_owner_is_rejected_before_parsing_or_reading_clock() {
    let expected = raw::source("Created", "Answer", 99, captured_at());
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("queries.accdb");
    std::fs::write(&path, &expected.before).unwrap();
    let clock = Arc::new(CountedClock(AtomicUsize::new(0)));
    // NO_CLONE: Retain the observer independently of the worker's injected clock.
    let worker = Worker::with_query_clock(|| {}, clock.clone());
    // NO_CLONE: The source path is independently retained for preservation checks.
    let Response::Opened {
        document_revision,
        queries,
        ..
    } = request(
        &worker,
        Command::Open {
            path: path.clone(),
            password: String::new(),
        },
    )
    .unwrap()
    else {
        panic!("expected real document");
    };
    assert!(matches!(
        request(
            &worker,
            Command::ReviewQueryCreation {
                target: QueryCreationTarget {
                    document_revision: document_revision + 1,
                    draft_revision: 1,
                    source: None
                },
                destination: "Created".into(),
                sql: "not SQL".into(),
            }
        ),
        Err(Error::StaleDocument)
    ));
    let saved = queries
        .into_iter()
        .find(|query| query.name == "Saved")
        .unwrap();
    assert!(matches!(
        request(
            &worker,
            Command::ReviewQueryCreation {
                target: QueryCreationTarget {
                    document_revision,
                    draft_revision: 2,
                    source: Some(SavedQueryTarget {
                        document_revision,
                        object_id: saved.object_id ^ 1,
                        name: saved.name,
                    })
                },
                destination: "Created".into(),
                sql: "not SQL".into(),
            }
        ),
        Err(Error::StaleSavedQuery)
    ));
    assert_eq!(clock.0.load(Ordering::SeqCst), 0);
    assert_eq!(std::fs::read(path).unwrap(), expected.before);
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
}

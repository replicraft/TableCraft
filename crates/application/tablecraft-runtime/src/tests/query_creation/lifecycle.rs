//! Actual source races and failed guarded reads retain their real resource owners.
use super::{CountedClock, captured_at};
use crate::tests::{request, terminal};
use crate::{
    Command, Error, QueryCreationOrigin, QueryCreationTarget, Response, SaveFailureKind,
    SaveOutcome, SaveWorker, SavedQueryTarget, Worker,
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use tablecraft_testkit::fixtures::access::saved_queries::creation as raw;

#[test]
fn changed_exclusive_source_retains_the_actual_actor_until_release_without_creating_query() {
    let case = raw::source("Created", "Answer", 99, captured_at());
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("queries.accdb");
    std::fs::write(&path, &case.before).unwrap();
    let clock = Arc::new(CountedClock(AtomicUsize::new(0)));
    // NO_CLONE: Worker and observer retain the same real provider so source-race
    // handling and explicit resource release cannot silently resample metadata.
    let worker = Worker::with_query_clock(|| {}, clock.clone());
    // NO_CLONE: The command owns the open path while the observer retains its
    // independent filesystem precondition and full-image oracle.
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
        panic!("expected source catalog");
    };
    let saved = queries
        .into_iter()
        .find(|query| query.name == "Saved")
        .unwrap();
    let Response::QueryCreationReviewed { review, capability } = request(
        &worker,
        Command::ReviewQueryCreation {
            target: QueryCreationTarget {
                document_revision,
                draft_revision: 17,
                source: Some(SavedQueryTarget {
                    document_revision,
                    object_id: saved.object_id,
                    name: saved.name,
                }),
            },
            sql: "SELECT 99 AS Answer;".into(),
            destination: "Created".into(),
        },
    )
    .unwrap() else {
        panic!("expected authored saved-editor review");
    };
    assert_eq!(clock.0.load(Ordering::SeqCst), 1);
    let QueryCreationOrigin::Draft {
        draft_revision,
        sql,
        source,
    } = review.origin
    else {
        panic!("dirty authored SQL must not become a stored-copy request");
    };
    assert_eq!(draft_revision, 17);
    assert_eq!(sql, "SELECT 99 AS Answer;");
    assert_eq!(source.as_ref().unwrap().name, "Saved");

    let mut changed = case.before;
    *changed.last_mut().unwrap() ^= 1;
    std::fs::write(&path, &changed).unwrap();
    let bundle = directory.path().join("creation.tcj");
    let saves = SaveWorker::new(|| {});
    // NO_CLONE: The actor owns its namespace path while the test independently
    // verifies that failure/release or successful ACK retires that exact bundle.
    let id = saves
        .start_query_creation(capability, bundle.clone())
        .unwrap();
    let report = terminal(&saves);
    assert_eq!(report.id, id);
    assert!(matches!(report.outcome, SaveOutcome::Failed(failure)
        if failure.kind == SaveFailureKind::SourceChanged && failure.can_release && !failure.can_recover));
    assert_eq!(saves.poll(), Some(report));
    assert!(!saves.acknowledge(id));
    assert!(saves.recover(id).is_err());
    assert!(!bundle.exists());
    assert_eq!(std::fs::read(&path).unwrap(), changed);

    let release = saves.release(id).unwrap();
    assert!(matches!(
        terminal(&saves).outcome,
        SaveOutcome::Released {
            current_outcome: None,
            last_completed: None,
        }
    ));
    assert!(saves.acknowledge(release));
    assert!(saves.poll().is_none());
    assert_eq!(std::fs::read(path).unwrap(), changed);
    assert_eq!(clock.0.load(Ordering::SeqCst), 1);
    assert_eq!(sql, "SELECT 99 AS Answer;");
    assert_eq!(source.unwrap().name, "Saved");
}

#[test]
fn saved_fact_cannot_publish_foreign_after_and_readonly_retry_never_replays_the_save() {
    let case = raw::source("Created", "Answer", 99, captured_at());
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("queries.accdb");
    std::fs::write(&path, &case.before).unwrap();
    let clock = Arc::new(CountedClock(AtomicUsize::new(0)));
    // NO_CLONE: The observer and actual worker independently retain the same
    // counted provider across review, save and both guarded read attempts.
    let worker = Worker::with_query_clock(|| {}, clock.clone());
    // NO_CLONE: Open owns its path; the test retains filesystem access to model
    // a real external change after the actual actor's completed save.
    let Response::Opened {
        document_revision, ..
    } = request(
        &worker,
        Command::Open {
            path: path.clone(),
            password: String::new(),
        },
    )
    .unwrap()
    else {
        panic!("expected source catalog");
    };
    let Response::QueryCreationReviewed { capability, .. } = request(
        &worker,
        Command::ReviewQueryCreation {
            target: QueryCreationTarget {
                document_revision,
                draft_revision: 21,
                source: None,
            },
            sql: "SELECT 99 AS Answer;".into(),
            destination: "Created".into(),
        },
    )
    .unwrap() else {
        panic!("expected creation review");
    };
    let witness = capability.reopen_witness();
    let saves = SaveWorker::new(|| {});
    let bundle = directory.path().join("creation.tcj");
    // NO_CLONE: The actor owns its namespace path while the test independently
    // verifies that failure/release or successful ACK retires that exact bundle.
    let id = saves
        .start_query_creation(capability, bundle.clone())
        .unwrap();
    let report = terminal(&saves);
    assert!(
        report
            .outcome
            .query_creation_reopen_facts()
            .unwrap()
            .replacement
    );
    assert_eq!(std::fs::read(&path).unwrap(), case.expected);
    assert!(saves.acknowledge(id));
    assert!(!bundle.exists());

    // NO_CLONE: The external writer owns a distinct mutation while the raw
    // AFTER remains the immutable oracle used to restore and verify read retry.
    let mut foreign = case.expected.clone();
    *foreign.last_mut().unwrap() ^= 1;
    std::fs::write(&path, &foreign).unwrap();
    // NO_CLONE: Failed publication consumes its command reference while the
    // caller retains the same immutable witness for an explicit read-only retry.
    assert!(matches!(
        request(
            &worker,
            Command::OpenQueryCreated {
                witness: Arc::clone(&witness)
            }
        ),
        Err(Error::QueryCreatedSourceChanged)
    ));
    assert_eq!(std::fs::read(&path).unwrap(), foreign);
    assert!(saves.poll().is_none());
    assert_eq!(clock.0.load(Ordering::SeqCst), 1);

    // Model an explicit external restoration for this test. Production retry
    // receives only the immutable read witness and never repairs/replays bytes.
    std::fs::write(&path, &case.expected).unwrap();
    let Response::Opened {
        document_revision: fresh,
        queries,
        ..
    } = request(&worker, Command::OpenQueryCreated { witness }).unwrap()
    else {
        panic!("expected explicit read-only retry");
    };
    assert!(fresh > document_revision);
    for (name, value) in [("Created", "99"), ("Saved", "42")] {
        let query = queries.iter().find(|query| query.name == name).unwrap();
        // NO_CLONE: The explicit Run command owns its selected fresh target;
        // the observed catalog remains independently available for the old query.
        let target = SavedQueryTarget {
            document_revision: fresh,
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
            panic!("expected actual saved Run");
        };
        assert_eq!(rows[0][0].text, value);
    }
    assert_eq!(std::fs::read(path).unwrap(), case.expected);
    assert_eq!(clock.0.load(Ordering::SeqCst), 1);
    assert!(saves.poll().is_none());
    assert!(!bundle.exists());
}

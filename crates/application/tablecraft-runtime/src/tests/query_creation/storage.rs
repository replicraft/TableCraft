//! Actual first-query save/reopen protects complete prior table membership.
use super::*;
use crate::QueryCreationStorageChange;
use tablecraft_testkit::fixtures::access::saved_queries::creation::initialization::{
    self as raw, Case,
};

fn run(case: Case) {
    let expected = raw::source(case, "Created", "Answer", 99, captured_at());
    let definition_page = u32::try_from(expected.before.len() / 4096).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("first-query.accdb");
    std::fs::write(&path, &expected.before).unwrap();
    let clock = Arc::new(CountedClock(AtomicUsize::new(0)));
    // NO_CLONE: The worker owns its injected clock while the observer checks
    // that save, ACK, guarded reopen and Run never read it again.
    let provider = Arc::clone(&clock);
    let worker = Worker::with_query_clock(|| {}, provider);
    // NO_CLONE: Open consumes its path; the test retains a separate file witness.
    let Response::Opened {
        document_revision,
        tables: prior,
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
        panic!("actual blank or protected local-table source");
    };
    assert!(prior.iter().all(|table| table.name != "MSysQueries"));

    let Response::QueryCreationReviewed { review, capability } = request(
        &worker,
        Command::ReviewQueryCreation {
            target: QueryCreationTarget {
                document_revision,
                draft_revision: 1,
                source: None,
            },
            sql: Arc::new("SELECT 99 AS [Answer];".into()),
            destination: "Created".into(),
        },
    )
    .unwrap() else {
        panic!("real source-bound creation review");
    };
    assert_eq!(review.native_id, expected.native_query_id);
    let witness = capability.reopen_witness();
    assert_eq!(
        witness.storage_change(),
        QueryCreationStorageChange::Initialized {
            native_table_id: i32::try_from(definition_page).unwrap(),
            definition_page,
        }
    );
    assert_eq!(clock.0.load(Ordering::SeqCst), 1);
    assert_eq!(std::fs::read(&path).unwrap(), expected.before);

    let saves = SaveWorker::new(|| {});
    let id = saves
        .start_query_creation(capability, directory.path().join("first-query.tcj"))
        .unwrap();
    let SaveOutcome::Saved { receipt } = terminal(&saves).outcome else {
        panic!("actual Created save");
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
    assert_eq!(std::fs::read(&path).unwrap(), expected.expected);
    assert!(saves.acknowledge(id));

    // NO_CLONE: The read worker owns its command witness while the test retains
    // the same immutable descriptor to verify the actual published table facts.
    let Response::Opened {
        document_revision: fresh_revision,
        tables,
        queries,
        ..
    } = request(
        &worker,
        Command::OpenQueryCreated {
            witness: Arc::clone(&witness),
        },
    )
    .unwrap()
    else {
        panic!("guarded complete-AFTER publication");
    };
    assert!(fresh_revision > document_revision);
    assert_eq!(tables.len(), prior.len() + 1);
    let initialized = tables
        .iter()
        .find(|table| table.name == "MSysQueries")
        .unwrap();
    assert!(initialized.local && initialized.system);
    assert_eq!(
        Some(initialized.storage.as_str()),
        witness.initialized_storage_display()
    );
    for (old, fresh) in prior
        .iter()
        .zip(tables.iter().filter(|table| table.name != "MSysQueries"))
    {
        assert_eq!(
            (&old.name, old.local, old.system, &old.storage),
            (&fresh.name, fresh.local, fresh.system, &fresh.storage)
        );
    }

    let created = queries
        .into_iter()
        .find(|query| query.name == "Created")
        .unwrap();
    let Response::Data { rows, .. } = request(
        &worker,
        Command::RunSavedQuery {
            target: SavedQueryTarget {
                document_revision: fresh_revision,
                object_id: created.object_id,
                name: created.name,
            },
            options: Default::default(),
            parameters: Vec::new(),
        },
    )
    .unwrap() else {
        panic!("explicit Run of actual stored QueryDef");
    };
    assert_eq!(rows[0][0].text, "99");
    assert_eq!(clock.0.load(Ordering::SeqCst), 1);
    assert_eq!(std::fs::read(path).unwrap(), expected.expected);
}

#[test]
fn first_query_in_a_real_blank_source_publishes_only_the_native_technical_addition() {
    run(Case::Blank);
}

#[test]
fn first_query_preserves_existing_ordinary_table_bytes_and_catalog_ownership() {
    run(Case::Ordinary);
}

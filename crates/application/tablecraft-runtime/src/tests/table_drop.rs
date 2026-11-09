//! Real table DROP actors retain source revision, receipt scope and complete reopen proof.
use super::{open, request, stored::opened, terminal};
use crate::{
    Command, Error, Response, ReviewedTableDrop, SaveFailureKind, SaveOutcome, SaveSchemaChange,
    SaveStartFailure, SaveWorker, TableDropReview, TableDropSourceInspection,
};
use tablecraft_testkit::fixtures::access::{canonical_creator_rename, canonical_table_drop as raw};

fn inspect(
    worker: &crate::Worker,
    revision: u64,
    table: &str,
) -> Result<TableDropSourceInspection, Error> {
    let response = request(
        worker,
        Command::InspectTableDropSource {
            document_revision: revision,
            table: table.into(),
        },
    )?;
    let Response::TableDropSourceInspected { inspection } = response else {
        panic!("expected read-only table DROP source facts");
    };
    Ok(inspection)
}

fn review(
    worker: &crate::Worker,
    revision: u64,
    table: &str,
) -> Result<(TableDropReview, ReviewedTableDrop), Error> {
    let response = request(
        worker,
        Command::ReviewTableDrop {
            document_revision: revision,
            table: table.into(),
        },
    )?;
    let Response::TableDropReviewed { review, capability } = response else {
        panic!("expected an actual consuming table DROP review");
    };
    assert_eq!(review.document_revision, revision);
    assert_eq!(
        review.document_revision,
        capability.captured_document_revision()
    );
    assert_eq!(review.table, capability.captured_table());
    assert_eq!(review.native_id, capability.captured_native_id());
    Ok((review, capability))
}

fn current_table(worker: &crate::Worker, table: &str, revision: u64) {
    let Response::Data {
        source_revision,
        rows,
        ..
    } = request(
        worker,
        Command::Page {
            table: table.into(),
            offset: 0,
        },
    )
    .unwrap()
    else {
        panic!("expected the unchanged current read session");
    };
    assert_eq!(source_revision, Some(revision));
    assert!(rows.is_empty());
}

#[test]
fn empty_inline_and_creator_sources_save_exact_after_then_publish_only_by_same_witness() {
    let pairs = [
        raw::unindexed(5, "Items", false),
        raw::unindexed(6, "雪🙂", false),
        raw::creator(&canonical_creator_rename::originals()[0]),
    ];
    for pair in pairs {
        let (directory, path, worker, revision) = opened(&pair.before);
        let inspected = inspect(&worker, revision, &pair.table).unwrap();
        assert!(inspected.source_eligible);
        assert_eq!(inspected.document_revision, revision);
        assert_eq!(inspected.table, pair.table);
        assert_eq!(inspected.declared_rows, 0);
        let (facts, capability) = review(&worker, revision, &pair.table).unwrap();
        let (_, second) = review(&worker, revision, &pair.table).unwrap();
        assert_eq!(facts.native_id, inspected.native_id);
        assert_eq!(facts.declared_rows, 0);
        assert_eq!(capability.source_path(), path);
        let witness = capability.reopen_witness();
        assert_eq!(witness.table_name(), pair.table);
        assert_eq!(witness.native_id(), facts.native_id);
        assert_eq!(witness.document_revision(), revision);
        assert_eq!(witness.expected_bytes(), pair.after);

        // Metadata and review hold no exclusive lease while confirmation is pending.
        tablecraft_access::editing::offline::OfflineWriteSession::open(
            &path,
            crate::controls::SCOPE,
            &mut crate::controls::budget(),
            &|| false,
        )
        .unwrap()
        .release()
        .unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), pair.before);
        let saves = SaveWorker::new(|| {});
        assert!(SaveWorker::supports_table_drop());
        let bundle = directory.path().join("drop.journal");
        let id = saves.start_table_drop(capability, bundle.clone()).unwrap();
        let busy = saves
            .start_table_drop(second, directory.path().join("second.journal"))
            .unwrap_err();
        assert_eq!(busy.kind, SaveStartFailure::Busy);
        assert!(busy.into_reviewed().is_some());
        let report = terminal(&saves);
        let SaveOutcome::Saved { receipt } = report.outcome else {
            panic!("expected actual table DROP completion: {report:?}");
        };
        assert_eq!(report.id, id);
        assert_eq!(receipt.schema_change, Some(SaveSchemaChange::TableDropped));
        assert_eq!((receipt.matched_rows, receipt.changed_rows), (None, None));
        assert!(receipt.query_change.is_none());
        assert!(receipt.database_options_change.is_none());
        assert!(
            report
                .outcome
                .table_drop_reopen_facts()
                .unwrap()
                .replacement
        );
        current_table(&worker, &pair.table, revision);
        assert!(saves.acknowledge(id));
        assert!(!bundle.exists());
        assert_eq!(std::fs::read(&path).unwrap(), pair.after);
        current_table(&worker, &pair.table, revision);

        let Response::Opened {
            document_revision: fresh,
            tables,
            queries,
            ..
        } = request(&worker, Command::OpenTableDropped { witness }).unwrap()
        else {
            panic!("expected checked complete-image table DROP reopen");
        };
        assert!(fresh > revision);
        assert!(tables.iter().all(|table| table.system));
        assert!(tables.iter().all(|table| table.name != pair.table));
        assert!(queries.is_empty());
        assert_eq!(std::fs::read(path).unwrap(), pair.after);
    }
}

#[test]
fn populated_source_and_wrong_revision_never_issue_a_drop_capability() {
    let pair = raw::unindexed(5, "Items", true);
    let (directory, path, worker, revision) = opened(&pair.before);
    let facts = inspect(&worker, revision, &pair.table).unwrap();
    assert!(!facts.source_eligible);
    assert_eq!(facts.declared_rows, 1);
    assert!(review(&worker, revision, &pair.table).is_err());
    assert!(matches!(
        inspect(&worker, revision + 1, &pair.table),
        Err(Error::StaleDocument)
    ));
    assert!(matches!(
        review(&worker, revision + 1, &pair.table),
        Err(Error::StaleDocument)
    ));
    assert!(matches!(
        inspect(&worker, revision, "items"),
        Err(Error::StaleTableDropTarget)
    ));
    assert!(review(&worker, revision, "items").is_err());
    assert_eq!(std::fs::read(path).unwrap(), pair.before);
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
}

#[test]
fn reopening_document_invalidates_consuming_review_before_any_save_actor_or_journal() {
    let pair = raw::unindexed(6, "Items", false);
    let (directory, path, worker, revision) = opened(&pair.before);
    let (_, capability) = review(&worker, revision, &pair.table).unwrap();
    request(&worker, open(&path, "")).unwrap();
    let saves = SaveWorker::new(|| {});
    let bundle = directory.path().join("stale.journal");
    let failure = saves
        .start_table_drop(capability, bundle.clone())
        .unwrap_err();
    assert_eq!(failure.kind, SaveStartFailure::StaleDocument);
    assert!(failure.into_reviewed().is_some());
    assert!(saves.poll().is_none());
    assert!(!bundle.exists());
    assert_eq!(std::fs::read(path).unwrap(), pair.before);
}

#[test]
fn original_cancelled_and_tampered_images_cannot_publish_a_table_drop_reopen() {
    let pair = raw::unindexed(5, "Items", false);
    let (directory, path, worker, revision) = opened(&pair.before);
    let (_, capability) = review(&worker, revision, &pair.table).unwrap();
    let witness = capability.reopen_witness();
    assert!(matches!(
        crate::document::Document::open_table_dropped(&witness, revision + 1, &|| false),
        Err(Error::TableDroppedSourceChanged)
    ));
    current_table(&worker, &pair.table, revision);
    let saves = SaveWorker::new(|| {});
    let id = saves
        .start_table_drop(capability, directory.path().join("after.journal"))
        .unwrap();
    assert!(matches!(
        terminal(&saves).outcome,
        SaveOutcome::Saved { .. }
    ));
    assert!(saves.acknowledge(id));
    assert!(matches!(
        crate::document::Document::open_table_dropped(&witness, revision + 1, &|| true),
        Err(Error::Cancelled)
    ));
    let mut foreign = pair.after;
    *foreign.last_mut().unwrap() ^= 1;
    std::fs::write(&path, &foreign).unwrap();
    assert!(matches!(
        request(&worker, Command::OpenTableDropped { witness }),
        Err(Error::TableDroppedSourceChanged)
    ));
    current_table(&worker, &pair.table, revision);
    assert_eq!(std::fs::read(path).unwrap(), foreign);
}

#[test]
fn source_drift_retains_real_release_owner_without_creating_journal_or_publishing_catalog() {
    let pair = raw::unindexed(5, "Items", false);
    let (directory, path, worker, revision) = opened(&pair.before);
    let (_, capability) = review(&worker, revision, &pair.table).unwrap();
    let mut foreign = pair.before;
    foreign[7 * 4096 + 3000] ^= 1;
    std::fs::write(&path, &foreign).unwrap();
    let saves = SaveWorker::new(|| {});
    let bundle = directory.path().join("refused.journal");
    let id = saves.start_table_drop(capability, bundle.clone()).unwrap();
    let SaveOutcome::Failed(failure) = terminal(&saves).outcome else {
        panic!("source drift must retain the actual offline resource owner");
    };
    assert_eq!(failure.kind, SaveFailureKind::SourceChanged);
    assert!(failure.can_release && !failure.can_recover);
    assert!(!bundle.exists());
    assert!(!saves.acknowledge(id));
    let release = saves.release(id).unwrap();
    assert!(matches!(
        terminal(&saves).outcome,
        SaveOutcome::Released { .. }
    ));
    assert!(saves.acknowledge(release));
    current_table(&worker, &pair.table, revision);
    assert_eq!(std::fs::read(path).unwrap(), foreign);
}

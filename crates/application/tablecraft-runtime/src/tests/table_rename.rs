//! Real reviewed catalog saves, retained failures and guarded fresh reopening.
use super::{open, request, stored::opened, terminal};
use crate::{
    Command, Error, Response, ReviewedTableRename, SaveFailureKind, SaveOutcome, SaveSchemaChange,
    SaveStartFailure, SaveWorker, TableRenameReview,
};
use tablecraft_core::values::Value;
use tablecraft_testkit::{
    fixtures::access::{
        canonical_catalog_names::rename_pair, canonical_indexed_rename,
        canonical_ordinary_indexed::Case,
    },
    support::access::retained_target::assert_retained_target_observation,
};

fn review(
    worker: &crate::Worker,
    document_revision: u64,
    old_name: &str,
    new_name: &str,
) -> (TableRenameReview, ReviewedTableRename) {
    let Response::TableRenameReviewed { review, capability } = request(
        worker,
        Command::ReviewTableRename {
            document_revision,
            table: old_name.into(),
            new_name: new_name.into(),
        },
    )
    .unwrap() else {
        panic!("expected a source-bound catalog rename review");
    };

    assert_eq!(review.document_revision, document_revision);
    assert_eq!(review.table, old_name);
    assert_eq!(review.new_name, new_name);
    (review, capability)
}

#[test]
fn reviewed_rename_saves_the_independent_image_and_reopens_without_record_counts() {
    for (header, indexed) in [
        (5, None),
        (6, None),
        (5, Some(Case::UniqueManual)),
        (6, Some(Case::CounterPrimary)),
        (5, Some(Case::AliasHistory)),
        (6, Some(Case::RetainedTree)),
    ] {
        let mut pair = rename_pair(header, "Items", "Renamed Ω", true);
        if let Some(case) = indexed {
            let source =
                canonical_indexed_rename::pair(header, case, &pair.old_name, &pair.new_name);
            pair.before = source.before;
            pair.after = source.after;
            pair.rows = source
                .rows
                .into_iter()
                .map(|row| {
                    row.into_iter()
                        .map(|value| match value {
                            Value::Int32(value) => value,
                            other => panic!("expected ordinary Int32 fixture cell: {other:?}"),
                        })
                        .collect()
                })
                .collect();
        }
        let (directory, path, worker, revision) = opened(&pair.before);
        let (review, capability) = review(&worker, revision, &pair.old_name, &pair.new_name);
        assert!(review.changed);
        assert_eq!(capability.source_path(), path);
        let witness = capability.reopen_witness();
        assert_eq!(witness.original_name(), pair.old_name);
        assert_eq!(witness.table_name(), pair.new_name);
        assert_eq!(witness.document_revision(), revision);

        // Typing and reviewing never retain the exclusive offline target owner.
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
        let bundle = directory.path().join("rename.journal");
        let id = saves
            .start_table_rename(capability, bundle.clone())
            .unwrap();
        let report = terminal(&saves);
        let SaveOutcome::Saved { receipt } = report.outcome else {
            panic!("expected completed catalog rename: {report:?}");
        };
        assert_eq!(receipt.schema_change, Some(SaveSchemaChange::TableRenamed));
        assert_eq!((receipt.matched_rows, receipt.changed_rows), (None, None));
        assert_eq!(saves.poll(), Some(report));
        assert!(saves.acknowledge(id));
        assert!(!bundle.exists());
        assert_eq!(std::fs::read(&path).unwrap(), pair.after);

        let Response::Opened { tables, .. } =
            request(&worker, Command::OpenTableRenamed { witness }).unwrap()
        else {
            panic!("expected guarded fresh renamed database");
        };
        assert!(tables.iter().any(|table| table.name == pair.new_name));
        assert!(!tables.iter().any(|table| table.name == pair.old_name));
        let Response::Data { rows, .. } = request(
            &worker,
            Command::Page {
                table: pair.new_name,
                offset: 0,
            },
        )
        .unwrap() else {
            panic!("expected preserved renamed-table rows");
        };
        let actual: Vec<Vec<String>> = rows
            .iter()
            .map(|row| row.iter().map(|cell| cell.text.clone()).collect())
            .collect();
        let expected: Vec<Vec<String>> = pair
            .rows
            .iter()
            .map(|row| row.iter().map(ToString::to_string).collect())
            .collect();
        assert_eq!(actual, expected);
    }
}

#[test]
fn stale_rename_retains_the_target_without_creating_a_journal() {
    let pair = rename_pair(5, "Items", "Renamed", true);
    let (directory, path, worker, revision) = opened(&pair.before);
    let (_, capability) = review(&worker, revision, &pair.old_name, &pair.new_name);
    let mut changed = pair.before;
    *changed.last_mut().unwrap() ^= 1;
    std::fs::write(&path, &changed).unwrap();

    let saves = SaveWorker::new(|| {});
    let bundle = directory.path().join("stale-rename.journal");
    let id = saves
        .start_table_rename(capability, bundle.clone())
        .unwrap();
    let SaveOutcome::Failed(failure) = terminal(&saves).outcome else {
        panic!("expected stale native source refusal");
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
        SaveOutcome::Released { .. }
    ));
    assert!(saves.acknowledge(released));
    assert_eq!(std::fs::read(&path).unwrap(), changed);
}

#[test]
fn document_replacement_invalidates_the_consuming_rename_capability() {
    let pair = rename_pair(5, "Items", "Renamed", false);
    let (directory, path, worker, revision) = opened(&pair.before);
    let (_, capability) = review(&worker, revision, &pair.old_name, &pair.new_name);
    request(&worker, open(&path, "")).unwrap();
    let saves = SaveWorker::new(|| {});
    let bundle = directory.path().join("invalidated-rename.journal");
    let error = saves
        .start_table_rename(capability, bundle.clone())
        .unwrap_err();
    assert_eq!(error.kind, SaveStartFailure::StaleDocument);
    assert!(error.into_reviewed().is_some());
    assert!(!bundle.exists());
    assert_eq!(std::fs::read(&path).unwrap(), pair.before);

    assert!(matches!(
        request(
            &worker,
            Command::ReviewTableRename {
                document_revision: revision,
                table: pair.old_name,
                new_name: pair.new_name,
            },
        ),
        Err(Error::StaleDocument)
    ));
}

#[test]
fn acknowledged_rename_witness_refuses_changed_content_without_replaying_the_action() {
    let pair = rename_pair(5, "Items", "Renamed", false);
    let (directory, path, worker, revision) = opened(&pair.before);
    let (_, capability) = review(&worker, revision, &pair.old_name, &pair.new_name);
    let witness = capability.reopen_witness();
    let saves = SaveWorker::new(|| {});
    let id = saves
        .start_table_rename(capability, directory.path().join("rename.journal"))
        .unwrap();
    assert!(matches!(
        terminal(&saves).outcome,
        SaveOutcome::Saved { .. }
    ));
    assert!(saves.acknowledge(id));

    let mut changed = pair.after;
    *changed.last_mut().unwrap() ^= 1;
    std::fs::write(&path, &changed).unwrap();
    assert!(matches!(
        request(&worker, Command::OpenTableRenamed { witness }),
        Err(Error::RenamedTableSourceChanged)
    ));
    assert_eq!(std::fs::read(&path).unwrap(), changed);
}

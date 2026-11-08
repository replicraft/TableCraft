//! Real unlocked column review, retained save ownership and guarded schema reopen.
use super::{open, request, stored::opened, terminal};
use crate::{
    AddColumnReview, Command, Error, Response, ReviewedAddColumn, SaveFailureKind, SaveOutcome,
    SaveSchemaChange, SaveStartFailure, SaveWorker,
};
use tablecraft_testkit::{
    fixtures::access::{
        canonical_add_column, canonical_created_add_column, canonical_creation,
        canonical_native17_creation,
    },
    support::access::retained_target::assert_retained_target_observation,
};

fn review(
    worker: &crate::Worker,
    revision: u64,
    column: &str,
) -> (AddColumnReview, ReviewedAddColumn) {
    let Response::AddColumnReviewed { review, capability } = request(
        worker,
        Command::ReviewAddInt32Column {
            document_revision: revision,
            table: "Items".into(),
            column: column.into(),
        },
    )
    .unwrap() else {
        panic!("expected source-qualified nullable Int32 column review");
    };
    assert_eq!(review.document_revision, revision);
    assert_eq!(review.table, "Items");
    assert_eq!(review.column, column);
    (review, capability)
}

#[test]
fn reviewed_column_saves_exact_sdk_image_and_reopens_full_schema_without_row_counts() {
    let keyless = canonical_add_column::source("Extra");
    let created = canonical_created_add_column::default_zero_source();
    let expected = canonical_created_add_column::append(&created, "Extra");
    for (before, after) in [(keyless.before, keyless.after), (created, expected)] {
        let (directory, path, worker, revision) = opened(&before);
        let (_, capability) = review(&worker, revision, "Extra");
        let (_, second) = review(&worker, revision, "Other");
        assert_eq!(capability.source_path(), path);
        let witness = capability.reopen_witness();
        assert_eq!(witness.table_name(), "Items");
        assert_eq!(witness.column_name(), "Extra");
        assert_eq!(witness.document_revision(), revision);
        tablecraft_access::editing::offline::OfflineWriteSession::open(
            &path,
            crate::controls::SCOPE,
            &mut crate::controls::budget(),
            &|| false,
        )
        .unwrap()
        .release()
        .unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), before);

        let saves = SaveWorker::new(|| {});
        let bundle = directory.path().join("column.journal");
        let id = saves.start_add_column(capability, bundle.clone()).unwrap();
        let rejected = saves
            .start_add_column(second, directory.path().join("other.journal"))
            .unwrap_err();
        assert_eq!(rejected.kind, SaveStartFailure::Busy);
        assert!(rejected.into_reviewed().is_some());
        let report = terminal(&saves);
        let SaveOutcome::Saved { receipt } = report.outcome else {
            panic!("expected completed source-qualified column save: {report:?}");
        };
        assert_eq!(receipt.schema_change, Some(SaveSchemaChange::ColumnAdded));
        assert_eq!((receipt.matched_rows, receipt.changed_rows), (None, None));
        assert_eq!(saves.poll(), Some(report));
        assert!(saves.acknowledge(id));
        assert!(!bundle.exists());
        assert_eq!(std::fs::read(&path).unwrap(), after);
        assert!(matches!(
            request(&worker, Command::OpenColumnAdded { witness }).unwrap(),
            Response::Opened { .. }
        ));
        let Response::Details {
            fields,
            declared_rows,
            ..
        } = request(
            &worker,
            Command::Describe {
                table: "Items".into(),
            },
        )
        .unwrap()
        else {
            panic!("expected newly opened schema");
        };
        assert_eq!(declared_rows, 0);
        assert_eq!(
            fields
                .iter()
                .map(|field| field.name.as_str())
                .collect::<Vec<_>>(),
            ["Id", "Score", "Extra"]
        );
        assert_eq!(fields[2].native_type, 4);
        assert_eq!(fields[2].reader_nullable, Some(true));
    }
}

#[test]
fn source_change_after_review_retains_target_without_journal_until_explicit_release() {
    let source = canonical_add_column::source("Extra");
    let (directory, path, worker, revision) = opened(&source.before);
    let (_, capability) = review(&worker, revision, "Extra");
    let mut changed = source.before;
    *changed.last_mut().unwrap() ^= 1;
    std::fs::write(&path, &changed).unwrap();
    let saves = SaveWorker::new(|| {});
    let bundle = directory.path().join("stale-column.journal");
    let id = saves.start_add_column(capability, bundle.clone()).unwrap();
    let SaveOutcome::Failed(failure) = terminal(&saves).outcome else {
        panic!("expected complete-source drift refusal");
    };
    assert_eq!(failure.kind, SaveFailureKind::SourceChanged);
    assert!(failure.can_release);
    assert!(!failure.can_recover);
    assert!(!saves.acknowledge(id));
    assert!(!bundle.exists());
    assert_retained_target_observation(&path, &changed);
    let release = saves.release(id).unwrap();
    assert!(matches!(
        terminal(&saves).outcome,
        SaveOutcome::Released { .. }
    ));
    assert!(saves.acknowledge(release));
    assert_eq!(std::fs::read(&path).unwrap(), changed);
}

#[test]
fn document_replacement_invalidates_the_consuming_column_capability_and_old_revision() {
    let source = canonical_add_column::source("Extra");
    let (directory, path, worker, revision) = opened(&source.before);
    let (_, capability) = review(&worker, revision, "Extra");
    request(&worker, open(&path, "")).unwrap();
    let saves = SaveWorker::new(|| {});
    let bundle = directory.path().join("invalidated.journal");
    let error = saves
        .start_add_column(capability, bundle.clone())
        .unwrap_err();
    assert_eq!(error.kind, SaveStartFailure::StaleDocument);
    assert!(error.into_reviewed().is_some());
    assert!(!bundle.exists());
    assert!(matches!(
        request(
            &worker,
            Command::ReviewAddInt32Column {
                document_revision: revision,
                table: "Items".into(),
                column: "Extra".into(),
            }
        ),
        Err(Error::StaleDocument)
    ));
    assert_eq!(std::fs::read(&path).unwrap(), source.before);
}

#[test]
fn column_reopen_witness_requires_complete_saved_content_without_replaying_addition() {
    let source = canonical_add_column::source("Extra");
    let (directory, path, worker, revision) = opened(&source.before);
    let (_, capability) = review(&worker, revision, "Extra");
    let witness = capability.reopen_witness();
    let saves = SaveWorker::new(|| {});
    let id = saves
        .start_add_column(capability, directory.path().join("column.journal"))
        .unwrap();
    assert!(matches!(
        terminal(&saves).outcome,
        SaveOutcome::Saved { .. }
    ));
    assert!(saves.acknowledge(id));
    let mut changed = source.after;
    *changed.last_mut().unwrap() ^= 1;
    std::fs::write(&path, &changed).unwrap();
    assert!(matches!(
        request(&worker, Command::OpenColumnAdded { witness }),
        Err(Error::AddedColumnSourceChanged)
    ));
    assert_eq!(std::fs::read(&path).unwrap(), changed);
}

#[test]
fn populated_sources_and_duplicate_declarations_fail_review_without_mutation() {
    for populated in [false, true] {
        let original = if populated {
            canonical_native17_creation::source(
                canonical_creation::source(canonical_creation::Case::One).bytes,
            )
        } else {
            canonical_add_column::source("Extra").before
        };
        let (directory, path, worker, revision) = opened(&original);
        assert!(matches!(
            request(
                &worker,
                Command::ReviewAddInt32Column {
                    document_revision: revision,
                    table: "Items".into(),
                    column: if populated { "Extra" } else { "sCoRe" }.into(),
                }
            ),
            Err(Error::AddColumn(_))
        ));
        assert_eq!(std::fs::read(&path).unwrap(), original);
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }
}

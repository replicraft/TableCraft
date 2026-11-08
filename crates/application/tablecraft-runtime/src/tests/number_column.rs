//! Shared runtime Number service consumes exact native Required(false)/zero proof.
use super::{open, request, stored::opened, terminal};
use crate::{
    AddColumnReview, Command, Error, FieldPropertyValue, Response, ReviewedNumberColumn,
    SaveFailureKind, SaveOutcome, SaveSchemaChange, SaveStartFailure, SaveWorker,
};
use tablecraft_testkit::fixtures::access::canonical_created_number as raw;

fn review(
    worker: &crate::Worker,
    revision: u64,
    table: &str,
    column: &str,
) -> (AddColumnReview, ReviewedNumberColumn) {
    let Response::NumberColumnReviewed { review, capability } = request(
        worker,
        Command::ReviewAddNumberColumn {
            document_revision: revision,
            table: table.into(),
            column: column.into(),
        },
    )
    .unwrap() else {
        panic!("expected explicit LONG Required(false)/DefaultValue0 review");
    };
    assert_eq!(review.document_revision, revision);
    assert_eq!(review.table, table);
    assert_eq!(review.column, column);
    (review, capability)
}

#[test]
fn number_review_save_and_full_reopen_match_all_three_independent_sdk_images() {
    for (case, table, column) in [
        (raw::Source::IdOnly, "Table1", "Score"),
        (raw::Source::GuiRequiredZero, "Table1", "Extra"),
        (raw::Source::DefaultOnly, "Items", "Extra"),
    ] {
        let before = raw::source(case);
        let after = raw::append(&before, case, column);
        let (directory, path, worker, revision) = opened(&before);
        let (_, capability) = review(&worker, revision, table, column);
        let (_, second) = review(&worker, revision, table, "Other");
        let witness = capability.reopen_witness();
        assert_eq!(witness.source_path(), path);
        assert_eq!(witness.table_name(), table);
        assert_eq!(witness.column_name(), column);
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
        let bundle = directory.path().join("number.journal");
        let id = saves
            .start_number_column(capability, bundle.clone())
            .unwrap();
        let busy = saves
            .start_number_column(second, directory.path().join("other.journal"))
            .unwrap_err();
        assert_eq!(busy.kind, SaveStartFailure::Busy);
        assert!(busy.into_reviewed().is_some());
        let report = terminal(&saves);
        let SaveOutcome::Saved { receipt } = report.outcome else {
            panic!("expected qualified Number save: {report:?}");
        };
        assert_eq!(receipt.schema_change, Some(SaveSchemaChange::ColumnAdded));
        assert_eq!((receipt.matched_rows, receipt.changed_rows), (None, None));
        assert_eq!(saves.poll(), Some(report));
        assert!(saves.acknowledge(id));
        assert!(!bundle.exists());
        assert_eq!(std::fs::read(&path).unwrap(), after);
        request(&worker, Command::OpenColumnAdded { witness }).unwrap();
        let Response::Details {
            fields,
            declared_rows,
            ..
        } = request(
            &worker,
            Command::Describe {
                table: table.into(),
            },
        )
        .unwrap()
        else {
            panic!("expected reopened Number declaration");
        };
        assert_eq!(declared_rows, 0);
        let field = fields.iter().find(|field| field.name == column).unwrap();
        assert_eq!(field.native_type, 4);
        assert_eq!(field.reader_nullable, Some(true));
        assert!(
            field
                .properties
                .iter()
                .any(|property| property.name == "Required"
                    && property.value == FieldPropertyValue::Boolean(false))
        );
        assert!(
            field
                .properties
                .iter()
                .any(|property| property.name == "DefaultValue"
                    && property.value == FieldPropertyValue::Text("0".into()))
        );
    }
}

#[test]
fn number_review_cannot_reuse_an_old_document_revision_or_consuming_capability() {
    let before = raw::source(raw::Source::IdOnly);
    let (directory, path, worker, revision) = opened(&before);
    let (_, capability) = review(&worker, revision, "Table1", "Score");
    request(&worker, open(&path, "")).unwrap();
    let saves = SaveWorker::new(|| {});
    let rejected = saves
        .start_number_column(capability, directory.path().join("stale.journal"))
        .unwrap_err();
    assert_eq!(rejected.kind, SaveStartFailure::StaleDocument);
    assert!(rejected.into_reviewed().is_some());
    assert!(matches!(
        request(
            &worker,
            Command::ReviewAddNumberColumn {
                document_revision: revision,
                table: "Table1".into(),
                column: "Score".into(),
            }
        ),
        Err(Error::StaleDocument)
    ));
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[test]
fn changed_number_after_refuses_whole_image_reopen_without_hiding_the_file() {
    let before = raw::source(raw::Source::IdOnly);
    let (_directory, path, worker, revision) = opened(&before);
    let (_, capability) = review(&worker, revision, "Table1", "Score");
    let witness = capability.reopen_witness();
    let mut changed = raw::append(&before, raw::Source::IdOnly, "Score");
    *changed.last_mut().unwrap() ^= 1;
    std::fs::write(&path, &changed).unwrap();
    assert!(matches!(
        request(&worker, Command::OpenColumnAdded { witness }),
        Err(Error::AddedColumnSourceChanged)
    ));
    assert_eq!(std::fs::read(&path).unwrap(), changed);
}

#[test]
fn changed_original_after_number_review_retains_owner_without_creating_a_journal() {
    let before = raw::source(raw::Source::IdOnly);
    let (directory, path, worker, revision) = opened(&before);
    let (_, capability) = review(&worker, revision, "Table1", "Score");
    let mut changed = before;
    *changed.last_mut().unwrap() ^= 1;
    std::fs::write(&path, &changed).unwrap();
    let saves = SaveWorker::new(|| {});
    let bundle = directory.path().join("drift.journal");
    let id = saves
        .start_number_column(capability, bundle.clone())
        .unwrap();
    let SaveOutcome::Failed(failure) = terminal(&saves).outcome else {
        panic!("expected fresh complete source refusal");
    };
    assert_eq!(failure.kind, SaveFailureKind::SourceChanged);
    assert!(failure.can_release);
    assert!(!failure.can_recover);
    assert!(!bundle.exists());
    assert!(!saves.acknowledge(id));
    let released = saves.release(id).unwrap();
    assert!(matches!(
        terminal(&saves).outcome,
        SaveOutcome::Released { .. }
    ));
    assert!(saves.acknowledge(released));
    assert_eq!(std::fs::read(&path).unwrap(), changed);
}

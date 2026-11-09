//! Same public Options route, selected native purpose and exact shared-page AFTER.
use super::{opened, request, review, terminal};
use crate::{
    Command, DatabaseOptionsInspection, Error, NameAutoCorrectState, Response,
    SaveDatabaseOptionsChange, SaveFailureKind, SaveOutcome, SaveWorker,
};
use tablecraft_ace::FieldHistoryOptionsIntentError;
use tablecraft_testkit::{
    fixtures::access::{
        canonical_created_number::{Source, mixed, repeated},
        canonical_database_options,
    },
    support::access::retained_target::assert_retained_target_observation,
};

const FIXTURE_PAGE_BYTES: usize = 4096;
const UNUSED_DEFINITION_GAP: usize = 7 * FIXTURE_PAGE_BYTES + 3000;

fn originals() -> Vec<Vec<u8>> {
    let first = repeated::source(Source::IdOnly);
    let text = mixed::append_text(
        &mixed::source(mixed::SourceCase::IdOnly),
        "Label",
        255,
        true,
    );
    let inline_text = mixed::append_text(
        &mixed::source(mixed::SourceCase::InlineNumber),
        "Label",
        255,
        true,
    );
    let number_after_text = mixed::append_number(&text, "AfterLabel");
    let mixed_history = mixed::append_text(&number_after_text, "OtherLabel", 255, true);
    vec![
        first.clone(),
        repeated::append(&first, "Score2"),
        text,
        inline_text,
        mixed_history,
    ]
}

fn inspect(worker: &crate::Worker, revision: u64) -> DatabaseOptionsInspection {
    let Response::DatabaseOptionsInspected { inspection } = request(
        worker,
        Command::InspectDatabaseOptions {
            document_revision: revision,
        },
    )
    .unwrap() else {
        panic!("actual native options observation expected");
    };
    assert_eq!(inspection.document_revision, revision);
    inspection
}

#[test]
fn normal_options_reviews_and_saves_all_five_field_history_sources_under_existing_pools() {
    assert!(SaveWorker::supports_database_options());
    for before in originals() {
        let expected = canonical_database_options::append_explicit_off_to_owned_page(&before);
        let (directory, path, worker, revision) = opened(&before);
        let observed = inspect(&worker, revision);
        assert_eq!(observed.name_auto_correct, NameAutoCorrectState::Missing);
        assert!(observed.disable_available);
        let (_, capability) = review(&worker, revision);
        let witness = capability.reopen_witness();
        assert_eq!(witness.source_path(), path);
        assert_eq!(witness.document_revision(), revision);
        // Both source observation and consuming review remain lease-free.
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
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
        let saves = SaveWorker::new(|| {});
        let bundle = directory.path().join("options.journal");
        let id = saves
            .start_disable_name_auto_correct(capability, bundle.clone())
            .unwrap();
        let report = terminal(&saves);
        assert_eq!(report.id, id);
        let SaveOutcome::Saved { receipt } = report.outcome else {
            panic!("expected the selected field-history native save: {report:?}");
        };
        assert_eq!(
            receipt.database_options_change,
            Some(SaveDatabaseOptionsChange::NameAutoCorrectDisabled)
        );
        assert_eq!((receipt.matched_rows, receipt.changed_rows), (None, None));
        assert_eq!((receipt.schema_change, receipt.query_change), (None, None));
        assert_eq!(saves.poll(), Some(report));
        assert!(saves.acknowledge(id));
        assert!(!bundle.exists());
        assert_eq!(std::fs::read(&path).unwrap(), expected);
        let Response::Opened {
            document_revision: fresh,
            ..
        } = request(&worker, Command::OpenDatabaseOptionsSaved { witness }).unwrap()
        else {
            panic!("expected the same accepted full-AFTER read witness");
        };
        assert_ne!(fresh, revision);
        let stored = inspect(&worker, fresh);
        assert_eq!(stored.name_auto_correct, NameAutoCorrectState::ExplicitOff);
        assert!(!stored.disable_available);
        assert_eq!(std::fs::read(path).unwrap(), expected);
    }
}

#[test]
fn field_history_source_drift_retains_target_then_release_uses_the_existing_save_owner() {
    let before = originals().remove(4);
    let (directory, path, worker, revision) = opened(&before);
    let (_, capability) = review(&worker, revision);
    let mut changed = before;
    assert_eq!(changed[UNUSED_DEFINITION_GAP], 0);
    changed[UNUSED_DEFINITION_GAP] = 1;
    std::fs::write(&path, &changed).unwrap();
    let saves = SaveWorker::new(|| {});
    let bundle = directory.path().join("options.journal");
    let id = saves
        .start_disable_name_auto_correct(capability, bundle.clone())
        .unwrap();
    let report = terminal(&saves);
    let SaveOutcome::Failed(failure) = report.outcome else {
        panic!("fresh full original must refuse drift: {report:?}");
    };
    assert_eq!(failure.kind, SaveFailureKind::SourceChanged);
    assert!(failure.can_release);
    assert!(!failure.can_recover);
    assert!(!saves.acknowledge(id));
    assert!(!bundle.exists());
    assert_retained_target_observation(&path, &changed);
    let release = saves.release(id).unwrap();
    let released = terminal(&saves);
    assert_eq!(released.id, release);
    assert!(matches!(released.outcome, SaveOutcome::Released { .. }));
    assert!(saves.acknowledge(release));
    assert_eq!(std::fs::read(path).unwrap(), changed);
}

#[test]
fn selected_field_history_refuses_noncanonical_source_and_cancel_without_other_purpose_retry() {
    let before = originals().remove(2);
    let mut changed = before.clone();
    assert_eq!(changed[UNUSED_DEFINITION_GAP], 0);
    changed[UNUSED_DEFINITION_GAP] = 1;
    let (_directory, path, worker, revision) = opened(&changed);
    let observed = inspect(&worker, revision);
    assert_eq!(observed.name_auto_correct, NameAutoCorrectState::Missing);
    assert!(!observed.disable_available);
    assert!(matches!(
        request(
            &worker,
            Command::ReviewDisableNameAutoCorrect {
                document_revision: revision
            }
        ),
        Err(Error::FieldHistoryDatabaseOptions(
            FieldHistoryOptionsIntentError::Options(_)
        ))
    ));
    assert_eq!(std::fs::read(&path).unwrap(), changed);
    let (mut document, _) =
        crate::document::Document::open(&path, &mut String::new(), revision, &|| false).unwrap();
    assert!(matches!(
        document.inspect_database_options(revision, &|| true),
        Err(Error::Cancelled)
    ));
    assert!(matches!(
        document.review_disable_name_auto_correct(revision, &|| true),
        Err(Error::Cancelled)
    ));
    assert_eq!(std::fs::read(path).unwrap(), changed);
}

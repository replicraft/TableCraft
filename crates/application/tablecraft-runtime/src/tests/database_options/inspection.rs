//! Stored facts stay separate from review eligibility and missing defaults.
use super::{opened, request};
use crate::{Command, DatabaseOptionsInspection, Error, NameAutoCorrectState, Response};
use tablecraft_testkit::fixtures::access::{
    canonical_creator_rename, canonical_database_options, database_properties,
};

fn inspect(worker: &crate::Worker, revision: u64) -> DatabaseOptionsInspection {
    let Response::DatabaseOptionsInspected { inspection } = request(
        worker,
        Command::InspectDatabaseOptions {
            document_revision: revision,
        },
    )
    .unwrap() else {
        panic!("expected inert database settings");
    };
    assert_eq!(inspection.document_revision, revision);
    inspection
}

#[test]
fn missing_absent_off_and_other_are_observed_without_invented_defaults() {
    let missing = canonical_creator_rename::originals().remove(0);
    let off = canonical_database_options::explicit_off(&missing);
    let other = database_properties::name_auto_correct_values(&[
        (1, 4, &1_i32.to_le_bytes()),
        (1, 4, &0_i32.to_le_bytes()),
    ]);
    for (before, expected, available) in [
        (missing, NameAutoCorrectState::Missing, true),
        (
            database_properties::database(None),
            NameAutoCorrectState::Absent,
            false,
        ),
        (off, NameAutoCorrectState::ExplicitOff, false),
        (
            database_properties::database(Some(&other)),
            NameAutoCorrectState::Other,
            false,
        ),
    ] {
        let (directory, path, worker, revision) = opened(&before);
        let facts = inspect(&worker, revision);
        assert_eq!(facts.name_auto_correct, expected);
        assert_eq!(facts.disable_available, available);
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }
}

#[test]
fn source_eligibility_holds_no_write_lease_and_old_revisions_are_refused() {
    let before = canonical_creator_rename::originals().remove(0);
    let (_directory, path, worker, revision) = opened(&before);
    assert!(inspect(&worker, revision).disable_available);
    tablecraft_access::editing::offline::OfflineWriteSession::open(
        &path,
        crate::controls::SCOPE,
        &mut crate::controls::budget(),
        &|| false,
    )
    .unwrap()
    .release()
    .unwrap();
    assert!(matches!(
        request(
            &worker,
            Command::InspectDatabaseOptions {
                document_revision: revision + 1
            }
        ),
        Err(Error::StaleDocument)
    ));
    assert_eq!(std::fs::read(path).unwrap(), before);
}

#[test]
fn cancelled_settings_inspection_returns_no_facts() {
    let before = canonical_creator_rename::originals().remove(0);
    let (_directory, path, _worker, revision) = opened(&before);
    let (mut document, _) =
        crate::document::Document::open(&path, &mut String::new(), revision, &|| false).unwrap();
    assert!(matches!(
        document.inspect_database_options(revision, &|| true),
        Err(Error::Cancelled)
    ));
    assert_eq!(std::fs::read(path).unwrap(), before);
}

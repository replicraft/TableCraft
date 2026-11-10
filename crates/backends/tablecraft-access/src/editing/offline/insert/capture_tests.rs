//! Capture-policy selection preserves typed native cancellation/resource failures.
use crate::editing::offline::{Cause, OfflineWriteSession, Phase, SessionState};
use std::fs;
use tablecraft_ace::InspectError;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{
    canonical_created_number::mixed::after_off::deleted_field::populated::{
        Case, readdition::Target as Field,
    },
    canonical_creation_defaults::{self as ordinary, Case as Ordinary},
    canonical_retained_add_table::{self as added, Target},
    canonical_table_drop::current as dropped,
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 100_000_000)
}

#[test]
fn insertion_capture_keeps_native_cancel_and_resource_causes_before_any_journal() {
    let original = dropped::source(
        Case::TextPresent,
        Field::ShortText,
        true,
        dropped::History::Fields,
    );
    let retired = dropped::after(&original);
    let retained = added::desktop::append(&retired, "Fresh", Target::GeneratedText);
    for (bytes, table) in [
        (ordinary::source(Ordinary::Absent).bytes, "Items"),
        (retained, "Fresh"),
    ] {
        for cancelled in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("target.accdb");
            fs::write(&path, &bytes).unwrap();
            let session =
                OfflineWriteSession::open(&path, CatalogScope::new(3121), &mut budget(), &|| false)
                    .unwrap();
            let mut exhausted = ResourceBudget::new(0, 0, 0);
            let Err(failure) = session.insert_table(table, &mut exhausted, &|| cancelled) else {
                panic!("capture unexpectedly succeeded");
            };
            assert!(matches!(failure.phase(), Phase::Planning));
            assert!(matches!(failure.state(), SessionState::RefreshRequired));
            if cancelled {
                assert!(matches!(
                    failure.cause(),
                    Cause::Native(InspectError::Cancelled)
                ));
            } else {
                assert!(matches!(
                    failure.cause(),
                    Cause::Native(InspectError::Limit(_))
                ));
            }
            failure.into_session().release().unwrap();
            assert_eq!(fs::read(&path).unwrap(), bytes);
            assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        }
    }
}

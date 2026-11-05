use super::*;
use std::{cell::Cell, fs};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{canonical_creation, canonical_native17_creation};

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

#[test]
fn viewed_image_precondition_retains_the_same_offline_editor_and_refuses_other_bytes() {
    let original = canonical_native17_creation::source(
        canonical_creation::source(canonical_creation::Case::One).bytes,
    );
    for (insert, change_tail) in [(false, false), (false, true), (true, false), (true, true)] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("source.mdb");
        let mut current = original.clone();
        if change_tail {
            *current.last_mut().unwrap() ^= 1;
        }
        fs::write(&path, &current).unwrap();
        let session =
            OfflineWriteSession::open(&path, CatalogScope::new(1), &mut budget(), &|| false)
                .unwrap();

        let admitted = if insert {
            session
                .insert_table_if_original("Items", &original, &mut budget(), &|| false)
                .map(|inserter| inserter.into_session())
        } else {
            session
                .edit_table_if_original("Items", &original, &mut budget(), &|| false)
                .map(|editor| editor.into_session())
        };
        let session = match admitted {
            Ok(session) => {
                assert!(!change_tail);
                session
            }
            Err(failure) => {
                assert!(change_tail);
                assert!(matches!(failure.cause(), Cause::SourceChanged));
                assert_eq!(failure.phase(), Phase::Planning);
                assert!(failure.invalid_input());
                assert!(failure.current_outcome().is_none());
                assert!(failure.pair_state().is_none());
                assert!(failure.setup_failure().is_none());
                failure.into_session()
            }
        };
        session.release().unwrap();
        assert_eq!(fs::read(&path).unwrap(), current);
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }
}

#[test]
fn original_comparison_checks_length_budget_and_cancellation_before_admission() {
    let bytes = vec![0xA7; 131_073];
    assert!(matches!(
        require_original(&bytes, &bytes[..bytes.len() - 1], &mut budget(), &|| false),
        Err(Cause::SourceChanged)
    ));

    let mut limited = ResourceBudget::new(2 * bytes.len() as u64 - 1, 0, 100);
    assert!(matches!(
        require_original(&bytes, &bytes, &mut limited, &|| false),
        Err(Cause::Native(InspectError::Limit(_)))
    ));

    let checks = Cell::new(0);
    let cancelled = || {
        let previous = checks.get();
        checks.set(previous + 1);
        previous >= 2
    };
    assert!(matches!(
        require_original(&bytes, &bytes, &mut budget(), &cancelled),
        Err(Cause::Native(InspectError::Cancelled))
    ));
}

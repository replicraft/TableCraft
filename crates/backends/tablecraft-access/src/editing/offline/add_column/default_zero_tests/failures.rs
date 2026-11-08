//! TCAC3 refusal preserves the same target owner and never invents a completed ADD.
use super::{budget, open, retained, review};
use crate::editing::offline::{Cause, Phase};
use std::fs;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::{IoError, SharedFile};
use tablecraft_testkit::{
    fixtures::access::canonical_created_number as raw,
    support::access::retained_target::assert_retained_target_observation,
};

#[test]
fn stored_false_staging_refuses_source_drift_native_proof_limits_and_cancellation_before_setup() {
    for case in 0..4 {
        let before = raw::source(raw::Source::IdOnly);
        let reviewed = review(&before, "Table1", "Score");
        let mut current = before;
        if case == 0 {
            *current.last_mut().unwrap() ^= 1;
        }
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("refused-number.accdb");
        let bundle = directory.path().join("journal");
        fs::write(&path, &current).unwrap();
        let mut native = if case == 1 {
            ResourceBudget::new(0, 0, 0)
        } else {
            budget()
        };
        let mut proof = if case == 2 {
            ResourceBudget::new(0, 0, 0)
        } else {
            budget()
        };
        let failure = open(&path)
            .stage_reviewed_default_zero_add_column_with_budgets(
                reviewed,
                &mut native,
                &mut proof,
                &|| case == 3,
            )
            .err()
            .expect("explicit-zero ADD must refuse before setup");
        assert_eq!(failure.phase(), Phase::Planning);
        if case == 0 {
            assert!(matches!(failure.cause(), Cause::SourceChanged));
        }
        assert!(failure.pair_state().is_none());
        assert!(failure.current_outcome().is_none());
        assert!(failure.last_completed().is_none());
        assert!(!bundle.exists());
        assert!(matches!(SharedFile::open(&path), Err(IoError::LockBusy)));
        assert_retained_target_observation(&path, &current);
        failure.into_session().release().unwrap();
        assert_eq!(fs::read(&path).unwrap(), current);
    }
}

#[test]
fn cancellation_or_persistence_limit_after_staging_preserves_original_length_and_owner() {
    for cancelled in [false, true] {
        let before = raw::source(raw::Source::IdOnly);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("unapplied-number.accdb");
        let bundle = directory.path().join("journal");
        fs::write(&path, &before).unwrap();
        let staged = open(&path)
            .stage_reviewed_default_zero_add_column_with_budgets(
                review(&before, "Table1", "Score"),
                &mut budget(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        let mut persistence = if cancelled {
            budget()
        } else {
            ResourceBudget::new(0, 0, 0)
        };
        let failure = staged
            .apply(&bundle, &mut persistence, &|| cancelled)
            .err()
            .expect("intent cannot gain setup authority after cancellation or limit");
        assert_eq!(failure.phase(), Phase::Intent);
        assert!(matches!(
            failure.cause(),
            Cause::DefaultZeroAddColumnIntent(_)
        ));
        assert!(failure.pair_state().is_none());
        assert!(failure.current_outcome().is_none());
        assert!(failure.last_completed().is_none());
        assert!(!bundle.exists());
        assert!(matches!(SharedFile::open(&path), Err(IoError::LockBusy)));
        assert_retained_target_observation(&path, &before);
        failure.into_session().release().unwrap();
        assert_eq!(fs::read(&path).unwrap(), before);
    }
}

#[test]
fn cancelled_or_exhausted_cold_open_retains_native_bytes_length_and_original_journal() {
    for committed in [false, true] {
        for cancelled in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("retained-number.accdb");
            let bundle = directory.path().join("journal");
            let expected = retained(&path, &bundle, committed, false);
            let mut work = if cancelled {
                budget()
            } else {
                ResourceBudget::new(0, 0, 0)
            };
            let failure = open(&path)
                .recover_update(&bundle, &mut work, &|| cancelled)
                .err()
                .expect("cold recovery must stop on its original work allowance");
            assert_eq!(failure.phase(), Phase::Setup);
            assert!(failure.setup_failure().is_some());
            assert!(bundle.exists());
            assert!(matches!(SharedFile::open(&path), Err(IoError::LockBusy)));
            assert_retained_target_observation(&path, &expected);
            failure.into_session().release().unwrap();
            assert_eq!(fs::read(&path).unwrap(), expected);
            assert!(bundle.exists());
        }
    }
}

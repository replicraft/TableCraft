//! TCAC4 runs through the same real V2 resource owner, without first-profile fallback.
use super::{RetainedPlan, SCOPE, budget, open, raw, retained_plan};
use crate::editing::offline::{
    Cause, CompletionKind, Phase, SchemaChange, VerifiedImage,
    plan::MutationIntent,
    test_support::{assert_completion, assert_staged_read_state},
};
use std::{fs, io::Cursor};
use tablecraft_ace::{NativeImage, RecoverableRepeatedDefaultZeroAddColumnPlan};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::{IoError, SharedFile, journal::RecoveryOutcome};

fn review(before: &[u8], table: &str, column: &str) -> RecoverableRepeatedDefaultZeroAddColumnPlan {
    NativeImage::capture(&mut Cursor::new(before), SCOPE, &mut budget(), &|| false)
        .unwrap()
        .plan_recoverable_repeated_default_zero_int32_column(table, column, &mut budget(), &|| {
            false
        })
        .unwrap()
}

#[test]
fn repeated_number_commits_all_three_independent_sdk_images_without_row_receipts() {
    for (case, table, column) in [
        (raw::Source::IdOnly, "Table1", "Score2"),
        (raw::Source::GuiRequiredZero, "Table1", "Extra2"),
        (raw::Source::DefaultOnly, "Items", "Extra2"),
    ] {
        let before = raw::repeated::source(case);
        let after = raw::repeated::append(&before, column);
        assert_eq!(before.len(), after.len());
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("repeated-number.accdb");
        let bundle = directory.path().join("journal");
        fs::write(&path, &before).unwrap();
        let staged = open(&path)
            .stage_reviewed_repeated_default_zero_add_column_with_budgets(
                review(&before, table, column),
                &mut budget(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!((staged.table(), staged.column()), (table, column));
        assert!(!bundle.exists());
        assert_staged_read_state(&path, &before);
        let (owner, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
        assert_completion(&receipt);
        assert_eq!(receipt.schema_change(), Some(SchemaChange::ColumnAdded));
        assert_eq!(
            (receipt.matched_rows(), receipt.changed_rows()),
            (None, None)
        );
        assert_eq!(receipt.query_change(), None);
        assert_eq!(receipt.database_options_change(), None);
        owner.release().unwrap();
        assert_eq!(fs::read(&path).unwrap(), after);
    }
}

#[test]
fn repeated_number_cold_and_settled_recovery_requalify_equal_length_v2_images() {
    for retired in [false, true] {
        for committed in [false, true] {
            let before = raw::repeated::source(raw::Source::IdOnly);
            let plan = review(&before, "Table1", "Score2");
            let intent = plan.intent().encode(&mut budget(), &|| false).unwrap();
            assert!(intent.starts_with(b"TCAC0004"));
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("recover-repeated.accdb");
            let bundle = directory.path().join("journal");
            let expected = retained_plan(
                &path,
                &bundle,
                RetainedPlan::Repeated(plan),
                committed,
                retired,
            );
            let (owner, receipt) = if retired {
                let retired_path = fs::read_dir(directory.path())
                    .unwrap()
                    .map(|entry| entry.unwrap().path())
                    .find(|path| {
                        path.file_name()
                            .unwrap()
                            .to_string_lossy()
                            .starts_with(".tablecraft-retired-")
                    })
                    .unwrap();
                open(&path)
                    .confirm_retired(&retired_path, &mut budget(), &|| false)
                    .unwrap()
            } else {
                open(&path)
                    .recover_update(&bundle, &mut budget(), &|| false)
                    .unwrap()
            };
            assert_eq!(
                receipt.kind(),
                if retired {
                    CompletionKind::Confirmed
                } else {
                    CompletionKind::Recovered
                }
            );
            assert_eq!(
                receipt.schema_change(),
                committed.then_some(SchemaChange::ColumnAdded)
            );
            assert_eq!(
                receipt.verified_image(),
                retired.then_some(if committed {
                    VerifiedImage::Replacement
                } else {
                    VerifiedImage::Original
                })
            );
            assert_eq!(
                receipt.outcome(),
                (!retired).then_some(if committed {
                    RecoveryOutcome::Committed
                } else {
                    RecoveryOutcome::RolledBack
                })
            );
            assert_eq!(
                (receipt.matched_rows(), receipt.changed_rows()),
                (None, None)
            );
            assert_eq!(receipt.query_change(), None);
            assert_eq!(receipt.database_options_change(), None);
            owner.release().unwrap();
            assert_eq!(fs::read(&path).unwrap(), expected);
        }
    }
}

#[test]
fn repeated_number_refusal_retains_original_owner_before_namespace_setup() {
    for case in 0..4 {
        let before = raw::repeated::source(raw::Source::IdOnly);
        let reviewed = review(&before, "Table1", "Score2");
        let mut current = before;
        if case == 0 {
            *current.last_mut().unwrap() ^= 1;
        }
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("refused-repeated.accdb");
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
            .stage_reviewed_repeated_default_zero_add_column_with_budgets(
                reviewed,
                &mut native,
                &mut proof,
                &|| case == 3,
            )
            .err()
            .expect("refuse before setup");
        assert_eq!(failure.phase(), Phase::Planning);
        if case == 0 {
            assert!(matches!(failure.cause(), Cause::SourceChanged));
        }
        assert!(failure.pair_state().is_none());
        assert!(failure.current_outcome().is_none());
        assert!(failure.last_completed().is_none());
        assert!(!bundle.exists());
        assert!(matches!(SharedFile::open(&path), Err(IoError::LockBusy)));
        failure.into_session().release().unwrap();
        assert_eq!(fs::read(&path).unwrap(), current);
    }
}

#[test]
fn malformed_version_four_never_uses_version_three_or_frozen_decoders() {
    for bytes in [b"TCAC0004".as_slice(), b"TCAC0004\0"] {
        assert!(matches!(
            MutationIntent::decode(bytes, &mut budget(), &|| false),
            Err(Cause::RepeatedDefaultZeroAddColumnIntent(_))
        ));
    }
    assert!(matches!(
        MutationIntent::decode(b"TCAC0003", &mut budget(), &|| false),
        Err(Cause::DefaultZeroAddColumnIntent(_))
    ));
    assert!(matches!(
        MutationIntent::decode(b"TCAC9999", &mut budget(), &|| false),
        Err(Cause::AddColumnIntent(_))
    ));
}

#[test]
fn repeated_intent_cancellation_or_limit_retains_owner_without_creating_a_bundle() {
    for cancelled in [false, true] {
        let before = raw::repeated::source(raw::Source::IdOnly);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("unapplied-repeated.accdb");
        let bundle = directory.path().join("journal");
        fs::write(&path, &before).unwrap();
        let staged = open(&path)
            .stage_reviewed_repeated_default_zero_add_column_with_budgets(
                review(&before, "Table1", "Score2"),
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
            .expect("refuse TCAC4 intent before setup");
        assert_eq!(failure.phase(), Phase::Intent);
        assert!(matches!(
            failure.cause(),
            Cause::RepeatedDefaultZeroAddColumnIntent(_)
        ));
        assert!(failure.pair_state().is_none());
        assert!(failure.current_outcome().is_none());
        assert!(failure.last_completed().is_none());
        assert!(!bundle.exists());
        assert!(matches!(SharedFile::open(&path), Err(IoError::LockBusy)));
        failure.into_session().release().unwrap();
        assert_eq!(fs::read(&path).unwrap(), before);
    }
}

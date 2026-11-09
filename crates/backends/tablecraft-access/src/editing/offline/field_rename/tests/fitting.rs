//! Fitted original requests use the same retained file/journal ownership protocol.
use super::*;
use crate::editing::offline::test_support::saved_field::{FieldPlan, retain_pair};
use tablecraft_io::journal::RecoveryOutcome;
use tablecraft_testkit::fixtures::access::saved_field_rename::fitting::{self, SourceCase};

fn fitted_review(before: &[u8], original: &str, replacement: &str) -> RecoverableFieldRenamePlan {
    NativeImage::capture(&mut Cursor::new(before), SCOPE, &mut budget(), &|| false)
        .unwrap()
        .plan_recoverable_fitted_field_rename(
            "Table1",
            original,
            replacement,
            &mut budget(),
            &|| false,
        )
        .unwrap()
}

#[test]
fn fitted_number_and_text_saves_match_complete_independent_images() {
    for case in [SourceCase::NumberFirst, SourceCase::TextNumberText] {
        for replacement in ["TaxAmount", "X"] {
            let original = fitting::old_name(case);
            let [before, after] = fitting::pair(case, replacement);
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("fitted.accdb");
            let bundle = directory.path().join("journal");
            fs::write(&path, &before).unwrap();
            let plan = fitted_review(&before, original, replacement);
            let staged = open(&path)
                .stage_reviewed_field_rename_with_budgets(
                    plan,
                    &mut budget(),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(staged.column(), original);
            assert_eq!(staged.new_name(), replacement);
            assert_eq!(staged.identity().column_number(), 1);
            assert_staged_read_state(&path, &before);
            assert!(!bundle.exists());
            let (owner, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
            assert_eq!(receipt.kind(), CompletionKind::Applied);
            assert_receipt(&receipt, true);
            assert_completion(&receipt);
            owner.release().unwrap();
            assert_eq!(fs::read(path).unwrap(), after);
        }
    }
}

#[test]
fn fitted_requests_recover_retained_and_cold_original_or_replacement_images() {
    for replacement in ["TaxAmount", "X"] {
        let [before, after] = fitting::pair(SourceCase::NumberHistory, replacement);
        let original = fitting::old_name(SourceCase::NumberHistory);
        for committed in [false, true] {
            for cold in [false, true] {
                let directory = tempfile::tempdir().unwrap();
                let path = directory.path().join("recover.accdb");
                let bundle = directory.path().join("journal");
                fs::write(&path, &before).unwrap();
                let owner = retain_pair(
                    open(&path),
                    &bundle,
                    FieldPlan::Rename(fitted_review(&before, original, replacement)),
                    committed,
                    false,
                    &mut budget(),
                );
                let (owner, receipt) = if cold {
                    owner.release().unwrap();
                    open(&path)
                        .recover_update(&bundle, &mut budget(), &|| false)
                        .unwrap()
                } else {
                    owner.recover_pending(&mut budget(), &|| false).unwrap()
                };
                assert_eq!(receipt.kind(), CompletionKind::Recovered);
                assert_eq!(
                    receipt.outcome(),
                    Some(if committed {
                        RecoveryOutcome::Committed
                    } else {
                        RecoveryOutcome::RolledBack
                    })
                );
                assert_receipt(&receipt, committed);
                assert_completion(&receipt);
                owner.release().unwrap();
                assert_eq!(
                    fs::read(&path).unwrap().as_slice(),
                    if committed {
                        after.as_slice()
                    } else {
                        before.as_slice()
                    }
                );
                assert!(!bundle.exists());
            }
        }
    }
}

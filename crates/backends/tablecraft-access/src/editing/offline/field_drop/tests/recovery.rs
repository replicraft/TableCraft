//! Same actual V1 pair through retained, cold and settled DROP recovery.
use super::*;
use crate::editing::offline::test_support::saved_field::{
    FieldPlan, retain_pair, retired_namespace,
};
use tablecraft_io::journal::RecoveryOutcome;

#[test]
fn retained_and_cold_drop_recovery_reconstruct_exact_original_requests_and_schema_scope() {
    for (case, column) in [
        (raw::SourceCase::NumberHistory, "Score"),
        (raw::SourceCase::MixedTextHistory, "Label"),
    ] {
        let before = raw::source(case);
        let after = raw::remove_nonprimary(&before, column, raw::Variant::AppendedGroupRemoved);
        for committed in [false, true] {
            for cold in [false, true] {
                let directory = tempfile::tempdir().unwrap();
                let path = directory.path().join("recover.accdb");
                let bundle = directory.path().join("journal");
                fs::write(&path, &before).unwrap();
                let owner = retain_pair(
                    open(&path),
                    &bundle,
                    FieldPlan::Drop(review(&before, column)),
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
                    fs::read(&path).unwrap(),
                    if committed { &after } else { &before }.as_slice()
                );
                assert!(!bundle.exists());
            }
        }
    }
}

#[test]
fn settled_drop_confirmation_has_no_replay_or_invented_historical_outcome() {
    let before = raw::source(raw::SourceCase::NumberHistory);
    for committed in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("confirmed.accdb");
        let bundle = directory.path().join("journal");
        fs::write(&path, &before).unwrap();
        let expected = if committed {
            raw::remove_nonprimary(&before, "Score", raw::Variant::AppendedGroupRemoved)
        } else {
            before.clone()
        };
        retain_pair(
            open(&path),
            &bundle,
            FieldPlan::Drop(review(&before, "Score")),
            committed,
            true,
            &mut budget(),
        )
        .release()
        .unwrap();
        let retired = retired_namespace(directory.path());
        let (owner, receipt) = open(&path)
            .confirm_retired(&retired, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(receipt.kind(), CompletionKind::Confirmed);
        assert_eq!(receipt.outcome(), None);
        assert_eq!(
            receipt.verified_image(),
            Some(if committed {
                VerifiedImage::Replacement
            } else {
                VerifiedImage::Original
            })
        );
        assert_receipt(&receipt, committed);
        owner.release().unwrap();
        assert_eq!(fs::read(&path).unwrap(), expected);
    }
}

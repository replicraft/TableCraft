//! Retained/cold/settled recovery shares the existing pair and receipt lifecycle.
use super::*;
use crate::editing::offline::test_support::saved_field::{
    FieldPlan, retain_pair, retired_namespace,
};
use tablecraft_io::journal::RecoveryOutcome;

#[test]
fn original_field_request_recovers_retained_and_cold_v1_pairs() {
    let before = raw::source(raw::SourceCase::Number);
    let after = review(&before, "Score", "Total").after_bytes().to_vec();
    for committed in [false, true] {
        for cold in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("recover.accdb");
            let bundle = directory.path().join("journal");
            fs::write(&path, &before).unwrap();
            let owner = retain_pair(
                open(&path),
                &bundle,
                FieldPlan::Rename(review(&before, "Score", "Total")),
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
                fs::read(path).unwrap(),
                if committed { &after } else { &before }.as_slice()
            );
        }
    }
}

#[test]
fn settled_confirmation_verifies_current_image_without_inventing_historical_outcome() {
    let before = raw::source(raw::SourceCase::Number);
    for committed in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("confirm.accdb");
        let bundle = directory.path().join("journal");
        fs::write(&path, &before).unwrap();
        retain_pair(
            open(&path),
            &bundle,
            FieldPlan::Rename(review(&before, "Score", "Total")),
            committed,
            true,
            &mut budget(),
        )
        .release()
        .unwrap();
        let retired = retired_namespace(directory.path());
        let expected = fs::read(&path).unwrap();
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
        assert_eq!(fs::read(path).unwrap(), expected);
    }
}

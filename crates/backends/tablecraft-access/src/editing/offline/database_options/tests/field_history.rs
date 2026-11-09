//! Actual retained files for the distinct original OFF-after-ADD request.
use super::*;
use tablecraft_ace::{FieldHistoryOptionsIntentError, RecoverableFieldHistoryDatabaseOptionsPlan};
use tablecraft_testkit::fixtures::access::canonical_created_number::{Source, mixed, repeated};

fn service_budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 5_000_000)
}

fn sources() -> [Vec<u8>; 2] {
    let text = mixed::append_text(
        &mixed::source(mixed::SourceCase::IdOnly),
        "Label",
        255,
        true,
    );
    let number = mixed::append_number(&text, "AfterLabel");
    [
        repeated::source(Source::IdOnly),
        mixed::append_text(&number, "OtherLabel", 255, true),
    ]
}

fn reviewed(before: &[u8]) -> RecoverableFieldHistoryDatabaseOptionsPlan {
    let image = NativeImage::capture(
        &mut Cursor::new(before),
        SCOPE,
        &mut service_budget(),
        &|| false,
    )
    .unwrap();
    let plan = image
        .plan_recoverable_disable_name_auto_correct_after_field_history(
            &mut service_budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(
        plan.after_bytes(),
        canonical_database_options::append_explicit_off_to_owned_page(before)
    );
    plan
}

#[test]
fn reviewed_field_history_off_uses_same_staged_owner_and_options_only_receipt() {
    for before in sources() {
        let after = canonical_database_options::append_explicit_off_to_owned_page(&before);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("field-history.accdb");
        let bundle = directory.path().join("journal");
        fs::write(&path, &before).unwrap();
        let staged = open(&path)
            .stage_reviewed_field_history_disable_name_auto_correct_with_budgets(
                reviewed(&before),
                &mut service_budget(),
                &mut service_budget(),
                &|| false,
            )
            .unwrap();
        assert!(!bundle.exists());
        assert_staged_read_state(&path, &before);
        let (owner, receipt) = staged
            .apply(&bundle, &mut service_budget(), &|| false)
            .unwrap();
        assert_eq!(receipt.kind(), CompletionKind::Applied);
        assert_options_receipt(&receipt, true);
        assert_completion(&receipt);
        owner.release().unwrap();
        assert_eq!(fs::read(&path).unwrap(), after);
    }
}

#[test]
fn abandoning_or_refusing_field_history_staging_keeps_original_and_resources() {
    let before = &sources()[0];
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("abandoned.accdb");
    fs::write(&path, before).unwrap();
    open(&path)
        .stage_reviewed_field_history_disable_name_auto_correct_with_budgets(
            reviewed(before),
            &mut service_budget(),
            &mut service_budget(),
            &|| false,
        )
        .unwrap()
        .into_session()
        .release()
        .unwrap();
    assert_eq!(fs::read(&path).unwrap(), *before);
    for case in 0..4 {
        let plan = reviewed(before);
        let mut current = before.clone();
        if case == 0 {
            current[4 * 4096 + 512] ^= 1;
        }
        fs::write(&path, &current).unwrap();
        let bundle = directory.path().join("journal");
        let mut native = if case == 1 {
            ResourceBudget::new(0, 0, 0)
        } else {
            service_budget()
        };
        let mut proof = if case == 2 {
            ResourceBudget::new(0, 0, 0)
        } else {
            service_budget()
        };
        let failure = open(&path)
            .stage_reviewed_field_history_disable_name_auto_correct_with_budgets(
                plan,
                &mut native,
                &mut proof,
                &|| case == 3,
            )
            .err()
            .expect("staging must refuse before journal creation");
        assert_eq!(failure.phase(), Phase::Planning);
        assert!(failure.pair_state().is_none());
        assert!(!bundle.exists());
        assert!(matches!(SharedFile::open(&path), Err(IoError::LockBusy)));
        failure.into_session().release().unwrap();
        assert_eq!(fs::read(&path).unwrap(), current);
    }
}

#[test]
fn retained_and_cold_field_history_recovery_keep_exact_v2_completion_facts() {
    let before = &sources()[0];
    let after = canonical_database_options::append_explicit_off_to_owned_page(before);
    for committed in [false, true] {
        for cold in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("recovery.accdb");
            let bundle = directory.path().join("journal");
            let owner = retained_pair(
                &path,
                &bundle,
                before,
                Plan::FieldHistory(reviewed(before)),
                committed,
                false,
            );
            let (owner, receipt) = if cold {
                owner.release().unwrap();
                open(&path)
                    .recover_update(&bundle, &mut service_budget(), &|| false)
                    .unwrap()
            } else {
                owner
                    .recover_pending(&mut service_budget(), &|| false)
                    .unwrap()
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
            assert_options_receipt(&receipt, committed);
            assert_completion(&receipt);
            owner.release().unwrap();
            assert_eq!(
                &fs::read(path).unwrap(),
                if committed { &after } else { before }
            );
        }
    }
}

#[test]
fn settled_field_history_confirmation_never_replays_the_settings_action() {
    let before = &sources()[1];
    let after = canonical_database_options::append_explicit_off_to_owned_page(before);
    for committed in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("confirmed.accdb");
        let bundle = directory.path().join("journal");
        retained_pair(
            &path,
            &bundle,
            before,
            Plan::FieldHistory(reviewed(before)),
            committed,
            true,
        )
        .release()
        .unwrap();
        let retired = retired_bundle(directory.path());
        let (owner, receipt) = open(&path)
            .confirm_retired(&retired, &mut service_budget(), &|| false)
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
        assert_options_receipt(&receipt, committed);
        assert_completion(&receipt);
        owner.release().unwrap();
        assert_eq!(
            &fs::read(path).unwrap(),
            if committed { &after } else { before }
        );
    }
}

#[test]
fn exact_new_version_is_selected_before_decode_and_never_retried_as_old() {
    for raw in [b"TCDO0002".as_slice(), b"TCDO0002\0"] {
        assert!(matches!(
            MutationIntent::decode(raw, &mut service_budget(), &|| false),
            Err(Cause::FieldHistoryDatabaseOptionsIntent(
                FieldHistoryOptionsIntentError::InvalidRecord
            ))
        ));
    }
    let before = &sources()[0];
    let raw = reviewed(before)
        .intent()
        .encode(&mut service_budget(), &|| false)
        .unwrap();
    assert_eq!(&raw[..8], b"TCDO0002");
    let mut wrong_profile = raw.clone();
    wrong_profile[12] = 1;
    assert!(matches!(
        MutationIntent::decode(&wrong_profile, &mut service_budget(), &|| false),
        Err(Cause::FieldHistoryDatabaseOptionsIntent(
            FieldHistoryOptionsIntentError::UnsupportedProfile
        ))
    ));
    let mut wrong_action = raw;
    wrong_action[56] = 2;
    assert!(matches!(
        MutationIntent::decode(&wrong_action, &mut service_budget(), &|| false),
        Err(Cause::FieldHistoryDatabaseOptionsIntent(
            FieldHistoryOptionsIntentError::InvalidRecord
        ))
    ));
}

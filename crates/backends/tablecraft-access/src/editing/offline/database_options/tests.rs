//! Reviewed OFF saves and explicit V2 recovery over actual retained file owners.
use super::*;
use crate::editing::offline::{
    CompletionKind, VerifiedImage,
    plan::MutationIntent,
    test_support::{assert_completion, assert_staged_read_state},
};
use std::{fs, io::Cursor, path::PathBuf};
use tablecraft_io::{
    IoError, SharedFile,
    journal::{RecoveryOutcome, filesystem::OfflineJournalPair},
};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{
    canonical_creation, canonical_creation_primary, canonical_database_options,
    canonical_native17_creation,
};

const SCOPE: CatalogScope = CatalogScope::new(1091);

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 1_000_000, 100_000_000)
}

fn original(indexed: bool) -> Vec<u8> {
    if indexed {
        canonical_creation_primary::source(canonical_creation_primary::Case::One).bytes
    } else {
        canonical_native17_creation::source(
            canonical_creation::source(canonical_creation::Case::One).bytes,
        )
    }
}

fn review(before: &[u8]) -> RecoverableNativeDatabaseOptionsPlan {
    let image =
        NativeImage::capture(&mut Cursor::new(before), SCOPE, &mut budget(), &|| false).unwrap();
    let plan = image
        .plan_recoverable_disable_name_auto_correct(&mut budget(), &|| false)
        .unwrap();
    assert_eq!(
        plan.after_bytes(),
        canonical_database_options::explicit_off(before)
    );
    plan
}

fn open(path: &Path) -> OfflineWriteSession {
    OfflineWriteSession::open(path, SCOPE, &mut budget(), &|| false).unwrap()
}

fn assert_options_receipt(receipt: &Receipt, changed: bool) {
    assert_eq!(
        receipt.database_options_change(),
        changed.then_some(DatabaseOptionsChange::NameAutoCorrectDisabled)
    );
    assert_eq!(
        (receipt.matched_rows(), receipt.changed_rows()),
        (None, None)
    );
    assert_eq!(receipt.schema_change(), None);
    assert_eq!(receipt.query_change(), None);
}

#[test]
fn reviewed_explicit_off_commits_complete_native_image_and_distinct_receipt() {
    for indexed in [false, true] {
        let before = original(indexed);
        let reviewed = review(&before);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("options.accdb");
        let bundle = directory.path().join("journal");
        fs::write(&path, &before).unwrap();
        let staged = open(&path)
            .stage_reviewed_disable_name_auto_correct_with_budgets(
                reviewed,
                &mut budget(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert!(!bundle.exists());
        assert_staged_read_state(&path, &before);
        let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
        assert_eq!(receipt.kind(), CompletionKind::Applied);
        assert_options_receipt(&receipt, true);
        assert_completion(&receipt);
        session.release().unwrap();
        assert_eq!(
            fs::read(&path).unwrap(),
            canonical_database_options::explicit_off(&before)
        );
    }
}

#[test]
fn discarding_review_preserves_the_same_original_without_a_journal() {
    let before = original(false);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("abandoned.accdb");
    let bundle = directory.path().join("journal");
    fs::write(&path, &before).unwrap();
    let staged = open(&path)
        .stage_reviewed_disable_name_auto_correct_with_budgets(
            review(&before),
            &mut budget(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
    staged.into_session().release().unwrap();
    assert_eq!(fs::read(&path).unwrap(), before);
    assert!(!bundle.exists());
}

fn retained_pair(
    path: &Path,
    bundle: &Path,
    before: &[u8],
    committed: bool,
    retired: bool,
) -> OfflineWriteSession {
    fs::write(path, before).unwrap();
    let plan = review(before);
    let raw = plan.intent().encode(&mut budget(), &|| false).unwrap();
    assert_eq!(&raw[..8], b"TCDO0001");
    let mut owner = open(path);
    let slot = owner.slot_mut().unwrap();
    let mut pair = OfflineJournalPair::create(
        slot.target.take().unwrap(),
        bundle,
        raw,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    {
        let (mut target, mut journal) = pair.resizable_storage().unwrap();
        let prepared = plan
            .prepare(&mut target, &mut journal, &mut budget(), &|| false)
            .unwrap();
        if committed {
            prepared.commit(&mut budget(), &|| false).unwrap();
        } else if retired {
            assert_eq!(
                prepared.rollback(&mut budget(), &|| false).unwrap(),
                RecoveryOutcome::RolledBack
            );
        }
    }
    if retired {
        pair.retire(if committed {
            RecoveryOutcome::Committed
        } else {
            RecoveryOutcome::RolledBack
        })
        .unwrap();
    }
    slot.pair = Some(pair);
    slot.state = SessionState::RetainedJournal;
    owner
}

#[test]
fn retained_and_cold_recovery_report_options_only_for_verified_commits() {
    let before = original(true);
    let after = canonical_database_options::explicit_off(&before);
    for committed in [false, true] {
        for cold in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("recovery.accdb");
            let bundle = directory.path().join("journal");
            let owner = retained_pair(&path, &bundle, &before, committed, false);
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
            assert_options_receipt(&receipt, committed);
            assert_completion(&receipt);
            owner.release().unwrap();
            let expected = if committed { &after } else { &before };
            assert_eq!(&fs::read(&path).unwrap(), expected);
        }
    }
}

fn retired_bundle(directory: &Path) -> PathBuf {
    let mut paths = fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(".tablecraft-retired-"))
        });
    let path = paths
        .next()
        .expect("one intact retired database-options journal");
    assert!(paths.next().is_none());
    path
}

#[test]
fn settled_confirmation_preserves_bytes_and_does_not_reapply_explicit_off() {
    let before = original(false);
    let after = canonical_database_options::explicit_off(&before);
    for committed in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("confirmed.accdb");
        let bundle = directory.path().join("journal");
        retained_pair(&path, &bundle, &before, committed, true)
            .release()
            .unwrap();
        let retired = retired_bundle(directory.path());
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
        assert_options_receipt(&receipt, committed);
        assert_completion(&receipt);
        owner.release().unwrap();
        let expected = if committed { &after } else { &before };
        assert_eq!(&fs::read(&path).unwrap(), expected);
    }
}

#[test]
fn stale_source_cancel_and_separate_pools_retain_the_owner_before_namespace_effects() {
    for case in 0..4 {
        let before = original(false);
        let reviewed = review(&before);
        let mut current = before;
        if case == 0 {
            current[4 * 4096 + 512] ^= 1;
        }
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("refused.accdb");
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
            .stage_reviewed_disable_name_auto_correct_with_budgets(
                reviewed,
                &mut native,
                &mut proof,
                &|| case == 3,
            )
            .err()
            .expect("OFF staging must refuse before journal setup");
        assert_eq!(failure.phase(), Phase::Planning);
        if case == 0 {
            assert!(matches!(failure.cause(), Cause::SourceChanged));
        }
        assert!(failure.pair_state().is_none());
        assert!(!bundle.exists());
        assert!(matches!(SharedFile::open(&path), Err(IoError::LockBusy)));
        failure.into_session().release().unwrap();
        assert_eq!(fs::read(&path).unwrap(), current);
    }
}

#[test]
fn malformed_options_namespace_never_falls_back_to_schema_or_row_decoders() {
    for raw in [b"TCDO".as_slice(), b"TCDO0001", b"TCDO0001\0", b"TCDO9999"] {
        assert!(matches!(
            MutationIntent::decode(raw, &mut budget(), &|| false),
            Err(Cause::DatabaseOptionsIntent(_))
        ));
    }
}

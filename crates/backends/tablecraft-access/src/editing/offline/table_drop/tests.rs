//! Real-file schema receipts and exact original-action recovery for table DROP.
use super::*;
use crate::editing::offline::{
    CompletionKind, SessionState, VerifiedImage,
    plan::MutationIntent,
    test_support::{assert_completion, assert_staged_read_state},
};
use std::{fs, io::Cursor};
use tablecraft_ace::{TableDropError, TableDropIntentError};
use tablecraft_io::{
    IoError, SharedFile,
    journal::{RecoveryOutcome, filesystem::OfflineJournalPair},
};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{
    canonical_creator_rename,
    canonical_table_drop::{self as raw, Pair},
};

const SCOPE: CatalogScope = CatalogScope::new(1210);

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 100_000_000)
}

fn review(pair: &Pair) -> RecoverableTableDropPlan {
    NativeImage::capture(
        &mut Cursor::new(&pair.before),
        SCOPE,
        &mut budget(),
        &|| false,
    )
    .unwrap()
    .plan_recoverable_table_drop(&pair.table, &mut budget(), &|| false)
    .unwrap()
}

fn open(path: &Path) -> OfflineWriteSession {
    OfflineWriteSession::open(path, SCOPE, &mut budget(), &|| false).unwrap()
}

fn assert_schema_receipt(receipt: &Receipt, dropped: bool) {
    assert_eq!(
        (receipt.matched_rows(), receipt.changed_rows()),
        (None, None)
    );
    assert_eq!(
        receipt.schema_change(),
        dropped.then_some(SchemaChange::TableDropped)
    );
    assert_eq!(receipt.query_change(), None);
    assert_eq!(receipt.database_options_change(), None);
}

#[test]
fn empty_native_profiles_stage_without_effects_then_report_only_table_dropped() {
    let creator = canonical_creator_rename::originals().remove(0);
    for pair in [
        raw::unindexed(5, "Items", false),
        raw::unindexed(6, "雪🙂", false),
        raw::creator(&creator),
    ] {
        let plan = review(&pair);
        assert_eq!(plan.after_bytes(), pair.after);
        let identity = plan.identity();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("drop.accdb");
        let journal = directory.path().join("journal");
        fs::write(&path, &pair.before).unwrap();

        let staged = open(&path)
            .stage_reviewed_table_drop_with_budgets(plan, &mut budget(), &mut budget(), &|| false)
            .unwrap();
        assert_eq!(staged.table(), pair.table);
        assert_eq!(staged.identity(), identity);
        assert_eq!(staged.declared_rows(), 0);
        assert_staged_read_state(&path, &pair.before);
        assert!(!journal.exists());

        let (owner, receipt) = staged.apply(&journal, &mut budget(), &|| false).unwrap();
        assert_eq!(receipt.kind(), CompletionKind::Applied);
        assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
        assert_schema_receipt(&receipt, true);
        assert_completion(&receipt);
        owner.release().unwrap();
        assert_eq!(fs::read(&path).unwrap(), pair.after);
        assert!(!journal.exists());
    }
}

#[test]
fn absent_and_populated_targets_refuse_from_actual_held_source_without_journaling() {
    for (pair, requested, missing) in [
        (raw::unindexed(5, "Items", false), "Absent", true),
        (raw::unindexed(6, "Items", true), "Items", false),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("refused.accdb");
        let journal = directory.path().join("journal");
        fs::write(&path, &pair.before).unwrap();
        let mut owner = open(&path);
        let slot = owner.slot_mut().unwrap();
        let image = NativeImage::capture(
            slot.target.as_mut().unwrap(),
            slot.scope,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let error = image
            .plan_recoverable_table_drop(requested, &mut budget(), &|| false)
            .err()
            .expect("unsupported source must not yield a checked table DROP plan");
        if missing {
            assert!(matches!(
                error,
                TableDropIntentError::Drop(TableDropError::MissingTable)
            ));
        } else {
            assert!(matches!(
                error,
                TableDropIntentError::Drop(TableDropError::Source)
            ));
        }
        assert!(!journal.exists());
        assert!(matches!(SharedFile::open(&path), Err(IoError::LockBusy)));
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        owner.release().unwrap();
        assert_eq!(fs::read(path).unwrap(), pair.before);
    }
}

#[test]
fn stale_review_retains_the_exact_target_owner_before_journal_setup() {
    let pair = raw::unindexed(5, "Items", false);
    let plan = review(&pair);
    let mut current = pair.before;
    *current.last_mut().unwrap() ^= 1;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("drift.accdb");
    let journal = directory.path().join("journal");
    fs::write(&path, &current).unwrap();

    let failure = open(&path)
        .stage_reviewed_table_drop_with_budgets(plan, &mut budget(), &mut budget(), &|| false)
        .err()
        .expect("the reviewed whole source must match before any journal exists");
    assert_eq!(failure.phase(), Phase::Planning);
    assert!(matches!(failure.cause(), Cause::SourceChanged));
    assert_eq!(failure.pair_state(), None);
    assert!(!journal.exists());
    assert!(matches!(SharedFile::open(&path), Err(IoError::LockBusy)));
    failure.into_session().release().unwrap();
    assert_eq!(fs::read(path).unwrap(), current);
}

#[test]
fn original_table_drop_intent_recovers_and_confirms_only_its_checked_schema_effect() {
    let pair = raw::unindexed(6, "Items", false);
    for committed in [false, true] {
        for settlement in 0..3 {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("recover.accdb");
            let journal = directory.path().join("journal");
            fs::write(&path, &pair.before).unwrap();
            let plan = review(&pair);
            let intent = plan.intent().encode(&mut budget(), &|| false).unwrap();
            assert!(intent.starts_with(b"TCTD0001"));

            let mut owner = open(&path);
            let slot = owner.slot_mut().unwrap();
            let mut retained = OfflineJournalPair::create(
                slot.target.take().unwrap(),
                &journal,
                intent,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            {
                let (mut target, mut storage) = retained.storage().unwrap();
                let prepared = plan
                    .prepare(&mut target, &mut storage, &mut budget(), &|| false)
                    .unwrap();
                if committed {
                    prepared.commit(&mut budget(), &|| false).unwrap();
                } else if settlement == 2 {
                    assert_eq!(
                        prepared.rollback(&mut budget(), &|| false).unwrap(),
                        RecoveryOutcome::RolledBack
                    );
                }
            }
            if settlement == 2 {
                retained
                    .retire(if committed {
                        RecoveryOutcome::Committed
                    } else {
                        RecoveryOutcome::RolledBack
                    })
                    .unwrap();
            }
            slot.pair = Some(retained);
            slot.state = SessionState::RetainedJournal;

            let (owner, receipt) = match settlement {
                0 => owner.recover_pending(&mut budget(), &|| false).unwrap(),
                1 => {
                    owner.release().unwrap();
                    open(&path)
                        .recover_update(&journal, &mut budget(), &|| false)
                        .unwrap()
                }
                2 => {
                    owner.release().unwrap();
                    let mut namespaces = fs::read_dir(directory.path())
                        .unwrap()
                        .map(|entry| entry.unwrap().path())
                        .filter(|path| path.is_dir());
                    let retired = namespaces.next().expect("the one retired journal bundle");
                    assert!(namespaces.next().is_none());
                    open(&path)
                        .confirm_retired(&retired, &mut budget(), &|| false)
                        .unwrap()
                }
                _ => unreachable!(),
            };
            if settlement == 2 {
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
            } else {
                assert_eq!(receipt.kind(), CompletionKind::Recovered);
                assert_eq!(
                    receipt.outcome(),
                    Some(if committed {
                        RecoveryOutcome::Committed
                    } else {
                        RecoveryOutcome::RolledBack
                    })
                );
            }
            assert_schema_receipt(&receipt, committed);
            assert_completion(&receipt);
            owner.release().unwrap();
            assert_eq!(
                fs::read(path).unwrap(),
                if committed { &pair.after } else { &pair.before }.as_slice()
            );
        }
    }
}

#[test]
fn malformed_table_drop_family_never_retries_row_or_field_decoders() {
    for bytes in [b"TCTD".as_slice(), b"TCTD0001", b"TCTD0001\0", b"TCTD0002"] {
        assert!(matches!(
            MutationIntent::decode(bytes, &mut budget(), &|| false),
            Err(Cause::TableDropIntent(TableDropIntentError::InvalidRecord))
        ));
    }
    assert!(matches!(
        MutationIntent::decode(b"TCRN0009", &mut budget(), &|| false),
        Err(Cause::TableRenameIntent(_))
    ));
}

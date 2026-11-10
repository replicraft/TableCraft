//! Real V3 owners preserve retained-table DROP receipts across cold recovery.
use super::{assert_schema_receipt, budget, open};
use crate::editing::offline::{
    Cause, CompletionKind, OfflineWriteSession, Phase, SessionState, VerifiedImage,
    test_support::{assert_completion, assert_staged_read_state, saved_field::retired_namespace},
};
use std::{fs, io::Cursor, path::Path};
use tablecraft_ace::{NativeImage, RecoverableTableDropPlan, inspect_catalog};
use tablecraft_io::{
    IoError, SharedFile,
    journal::{RecoveryOutcome, filesystem::OfflineJournalPair},
};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_table_drop::retained as raw;

const SCOPE: CatalogScope = CatalogScope::new(1211);

fn review(before: &[u8]) -> RecoverableTableDropPlan {
    let plan =
        NativeImage::capture_retained(&mut Cursor::new(before), SCOPE, &mut budget(), &|| false)
            .unwrap()
            .plan_recoverable_table_drop("Table1", &mut budget(), &|| false)
            .unwrap();
    assert!(plan.intent().is_retained());
    plan
}

#[test]
fn retained_tables_stage_without_namespace_then_publish_exact_drop_and_schema_receipt() {
    for case in raw::CASES {
        let source = raw::source(case);
        let after = raw::after(
            &source.bytes,
            source.definition,
            source.catalog_root,
            &source.roots,
        );
        let plan = review(&source.bytes);
        assert_eq!(plan.after_bytes(), after);
        assert_eq!(plan.declared_rows() as usize, source.roots.len());
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("retained.accdb");
        let bundle = directory.path().join("journal");
        fs::write(&path, &source.bytes).unwrap();
        let staged = open(&path)
            .stage_reviewed_table_drop_with_budgets(plan, &mut budget(), &mut budget(), &|| false)
            .unwrap();
        assert_eq!(staged.table(), "Table1");
        assert_eq!(staged.identity().definition_page(), source.definition);
        assert_eq!(staged.declared_rows() as usize, source.roots.len());
        assert_staged_read_state(&path, &source.bytes);
        assert!(!bundle.exists());
        let (owner, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
        assert_schema_receipt(&receipt, true);
        assert_eq!(receipt.kind(), CompletionKind::Applied);
        assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
        assert_completion(&receipt);
        owner.release().unwrap();
        assert_eq!(fs::read(&path).unwrap(), after);
        let catalog =
            inspect_catalog(&mut Cursor::new(&after), SCOPE, &mut budget(), &|| false).unwrap();
        assert!(
            catalog
                .catalog()
                .objects()
                .iter()
                .all(|object| object.name().as_str() != "Table1")
        );
        assert!(
            catalog
                .catalog()
                .objects()
                .iter()
                .any(|object| object.name().as_str() == "Other")
        );
    }
}

fn interrupted(
    mut owner: OfflineWriteSession,
    bundle: &Path,
    plan: RecoverableTableDropPlan,
    committed: bool,
    retired: bool,
) -> OfflineWriteSession {
    let wire = plan.intent().encode(&mut budget(), &|| false).unwrap();
    assert!(wire.starts_with(b"TCTD0003"));
    let slot = owner.slot_mut().unwrap();
    let mut pair = OfflineJournalPair::create(
        slot.target.take().unwrap(),
        bundle,
        wire,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    {
        let (mut target, mut journal) = pair.resizable_storage().unwrap();
        let prepared = plan
            .prepare_retained(&mut target, &mut journal, &mut budget(), &|| false)
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
fn retained_table_drop_v3_recovery_and_settled_confirmation_keep_schema_effects_exact() {
    let source = raw::source(raw::Case::SecondText);
    let after = raw::after(
        &source.bytes,
        source.definition,
        source.catalog_root,
        &source.roots,
    );
    for committed in [false, true] {
        for settlement in 0..3 {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("recover.accdb");
            let bundle = directory.path().join("journal");
            fs::write(&path, &source.bytes).unwrap();
            let owner = interrupted(
                open(&path),
                &bundle,
                review(&source.bytes),
                committed,
                settlement == 2,
            );
            let (owner, receipt) = match settlement {
                0 => owner.recover_pending(&mut budget(), &|| false).unwrap(),
                1 => {
                    owner.release().unwrap();
                    open(&path)
                        .recover_update(&bundle, &mut budget(), &|| false)
                        .unwrap()
                }
                2 => {
                    owner.release().unwrap();
                    open(&path)
                        .confirm_retired(
                            &retired_namespace(directory.path()),
                            &mut budget(),
                            &|| false,
                        )
                        .unwrap()
                }
                _ => unreachable!(),
            };
            assert_schema_receipt(&receipt, committed);
            assert_completion(&receipt);
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
            owner.release().unwrap();
            assert_eq!(
                fs::read(&path).unwrap(),
                if committed { &after } else { &source.bytes }.as_slice()
            );
        }
    }
}

#[test]
fn retained_table_drop_source_drift_retains_same_exclusive_owner_without_journal_effects() {
    let source = raw::source(raw::Case::FirstNumber);
    let plan = review(&source.bytes);
    let mut current = source.bytes;
    *current.last_mut().unwrap() ^= 1;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("drift.accdb");
    let bundle = directory.path().join("journal");
    fs::write(&path, &current).unwrap();
    let failure = open(&path)
        .stage_reviewed_table_drop_with_budgets(plan, &mut budget(), &mut budget(), &|| false)
        .err()
        .expect("whole retained BEFORE must match before journal setup");
    assert_eq!(failure.phase(), Phase::Planning);
    assert!(matches!(failure.cause(), Cause::SourceChanged));
    assert_eq!(failure.pair_state(), None);
    assert!(!bundle.exists());
    assert!(matches!(SharedFile::open(&path), Err(IoError::LockBusy)));
    failure.into_session().release().unwrap();
    assert_eq!(fs::read(path).unwrap(), current);
}

//! Real target ownership and exact TCRN recovery over independent native images.
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
    canonical_catalog_names::{RenamePair, rename_pair},
    canonical_indexed_rename,
    canonical_ordinary_indexed::Case,
};

const SCOPE: CatalogScope = CatalogScope::new(591);

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

fn capture(before: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(before), SCOPE, &mut budget(), &|| false).unwrap()
}

fn review(source: &RenamePair) -> RecoverableNativeTableRenamePlan {
    capture(&source.before)
        .plan_recoverable_table_rename(&source.old_name, &source.new_name, &mut budget(), &|| false)
        .unwrap()
}

fn open(path: &Path) -> OfflineWriteSession {
    OfflineWriteSession::open(path, SCOPE, &mut budget(), &|| false).unwrap()
}

#[test]
fn unicode_growth_shrink_and_case_changes_preserve_complete_source_and_schema_receipts() {
    for (header, old, new, populated) in [
        (5, "Items", "雪🙂 Customers", true),
        (6, "Long ordinary table", "短", false),
        (5, "Items", "items", true),
        (6, "Items", "Items", true),
        (6, "\u{feff}雪", "\u{feff}A🙂", true),
    ] {
        let source = rename_pair(header, old, new, populated);
        let reviewed = review(&source);
        assert_eq!(reviewed.after_bytes(), source.after);
        let identity = reviewed.identity();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("rename.accdb");
        let bundle = directory.path().join("journal");
        fs::write(&path, &source.before).unwrap();

        let staged = open(&path)
            .stage_reviewed_table_rename_with_budgets(
                reviewed,
                &mut budget(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(staged.table(), old);
        assert_eq!(staged.new_name(), new);
        assert_eq!(staged.identity(), identity);
        assert_eq!(staged.changed(), old != new);
        assert_staged_read_state(&path, &source.before);
        assert!(!bundle.exists());

        let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
        assert_eq!(
            (receipt.matched_rows(), receipt.changed_rows()),
            (None, None)
        );
        assert_eq!(
            receipt.schema_change(),
            (old != new).then_some(SchemaChange::TableRenamed)
        );
        assert_completion(&receipt);
        session.release().unwrap();
        assert_eq!(fs::read(&path).unwrap(), source.after);
    }
}

#[test]
fn source_drift_cancellation_and_independent_pool_exhaustion_keep_target_without_journal() {
    for case in 0..4 {
        let source = rename_pair(6, "Items", "Renamed", true);
        let reviewed = review(&source);
        let mut current = source.before;
        if case == 0 {
            *current.last_mut().unwrap() ^= 1;
        }
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("refused.accdb");
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
            .stage_reviewed_table_rename_with_budgets(reviewed, &mut native, &mut proof, &|| {
                case == 3
            })
            .err()
            .expect("rename must refuse before journal setup");
        if case == 0 {
            assert!(matches!(failure.cause(), Cause::SourceChanged));
        }
        assert_eq!(failure.phase(), Phase::Planning);
        assert_eq!(failure.pair_state(), None);
        assert!(matches!(SharedFile::open(&path), Err(IoError::LockBusy)));
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        failure.into_session().release().unwrap();
        assert_eq!(fs::read(&path).unwrap(), current);
    }
}

fn retained_pair(
    path: &Path,
    bundle: &Path,
    before: &[u8],
    plan: RecoverableNativeTableRenamePlan,
    committed: bool,
    retired: bool,
) -> OfflineWriteSession {
    fs::write(path, before).unwrap();
    let raw = plan.intent().encode(&mut budget(), &|| false).unwrap();
    assert!(raw.starts_with(b"TCRN"));
    let mut session = open(path);
    let slot = session.slot_mut().unwrap();
    let mut pair = OfflineJournalPair::create(
        slot.target.take().unwrap(),
        bundle,
        raw,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    {
        let (mut target, mut journal) = pair.storage().unwrap();
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
    session
}

#[test]
fn retained_and_cold_recovery_dispatch_original_tcrn_and_report_only_real_schema_effect() {
    let source = rename_pair(5, "Items", "雪 Renamed", true);
    for committed in [false, true] {
        for cold in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("recover.accdb");
            let bundle = directory.path().join("journal");
            let owner = retained_pair(
                &path,
                &bundle,
                &source.before,
                review(&source),
                committed,
                false,
            );
            let (session, receipt) = if cold {
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
            assert_eq!(
                (receipt.matched_rows(), receipt.changed_rows()),
                (None, None)
            );
            assert_eq!(
                receipt.schema_change(),
                committed.then_some(SchemaChange::TableRenamed)
            );
            assert_completion(&receipt);
            session.release().unwrap();
            assert_eq!(
                fs::read(&path).unwrap(),
                if committed {
                    &source.after
                } else {
                    &source.before
                }
                .as_slice()
            );
        }
    }
}

fn retired_bundle(directory: &Path) -> PathBuf {
    let mut entries = fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(".tablecraft-retired-"))
        });
    let path = entries.next().expect("expected retained journal namespace");
    assert!(entries.next().is_none());
    path
}

#[test]
fn settled_confirmation_does_not_write_or_invent_row_counts() {
    let source = rename_pair(6, "Items", "Renamed", true);
    for committed in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("confirm.accdb");
        let bundle = directory.path().join("journal");
        retained_pair(
            &path,
            &bundle,
            &source.before,
            review(&source),
            committed,
            true,
        )
        .release()
        .unwrap();
        let retired = retired_bundle(directory.path());
        let before = fs::read(&path).unwrap();
        let (session, receipt) = open(&path)
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
        assert_eq!(
            (receipt.matched_rows(), receipt.changed_rows()),
            (None, None)
        );
        assert_eq!(
            receipt.schema_change(),
            committed.then_some(SchemaChange::TableRenamed)
        );
        session.release().unwrap();
        assert_eq!(fs::read(&path).unwrap(), before);
    }
}

#[test]
fn malformed_rename_namespace_never_falls_back_to_row_or_creation_decoders() {
    for raw in [b"TCRN".as_slice(), b"TCRN0001", b"TCRN0001\0", b"TCRN0002"] {
        assert!(matches!(
            MutationIntent::decode(raw, &mut budget(), &|| false),
            Err(Cause::TableRenameIntent(_))
        ));
    }
}

#[test]
fn indexed_review_commit_and_cold_recovery_use_the_v2_native_profile() {
    for (header, case) in [
        (5, Case::CounterPrimary),
        (6, Case::AliasHistory),
        (5, Case::RetainedTree),
        (6, Case::NullableOrdinary),
    ] {
        let source = canonical_indexed_rename::pair(header, case, "Items", "Renamed Ω");
        let review = || {
            let plan = capture(&source.before)
                .plan_recoverable_indexed_table_rename(
                    "Items",
                    "Renamed Ω",
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(plan.after_bytes(), source.after);
            assert_eq!(
                &plan.intent().encode(&mut budget(), &|| false).unwrap()[..8],
                b"TCRN0002"
            );
            plan
        };
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("indexed.accdb");
        let bundle = directory.path().join("journal");
        fs::write(&path, &source.before).unwrap();

        let staged = open(&path)
            .stage_reviewed_table_rename_with_budgets(
                review(),
                &mut budget(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_staged_read_state(&path, &source.before);
        let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
        assert_eq!(receipt.schema_change(), Some(SchemaChange::TableRenamed));
        assert_eq!(
            (receipt.matched_rows(), receipt.changed_rows()),
            (None, None)
        );
        assert_completion(&receipt);
        session.release().unwrap();
        assert_eq!(fs::read(&path).unwrap(), source.after);

        for committed in [false, true] {
            for settled in [false, true] {
                let recovery = tempfile::tempdir().unwrap();
                let path = recovery.path().join("recover.accdb");
                let bundle = recovery.path().join("journal");
                let owner =
                    retained_pair(&path, &bundle, &source.before, review(), committed, settled);
                owner.release().unwrap();
                let reopened = open(&path);
                let (session, receipt) = if settled {
                    reopened
                        .confirm_retired(&retired_bundle(recovery.path()), &mut budget(), &|| false)
                        .unwrap()
                } else {
                    reopened
                        .recover_update(&bundle, &mut budget(), &|| false)
                        .unwrap()
                };
                if settled {
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
                assert_eq!(
                    receipt.schema_change(),
                    committed.then_some(SchemaChange::TableRenamed)
                );
                assert_eq!(
                    (receipt.matched_rows(), receipt.changed_rows()),
                    (None, None)
                );
                assert_completion(&receipt);
                session.release().unwrap();
                assert_eq!(
                    fs::read(&path).unwrap(),
                    if committed {
                        &source.after
                    } else {
                        &source.before
                    }
                    .as_slice()
                );
            }
        }
    }
}

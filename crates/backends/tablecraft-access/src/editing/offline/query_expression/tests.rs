//! Retained-target query metadata saves and closed TCQE lifecycle observations.
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
use tablecraft_testkit::fixtures::access::saved_queries::fitting_select::storage_pair;

const SCOPE: CatalogScope = CatalogScope::new(992);

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 1_000_000, 500_000_000)
}

fn review(before: &[u8], replacement: &str) -> RecoverableNativeQueryExpressionPlan {
    NativeImage::capture(&mut Cursor::new(before), SCOPE, &mut budget(), &|| false)
        .unwrap()
        .plan_recoverable_query_literal_expression(
            "Saved",
            "42",
            replacement,
            &mut budget(),
            &|| false,
        )
        .unwrap()
}

fn open(path: &Path) -> OfflineWriteSession {
    OfflineWriteSession::open(path, SCOPE, &mut budget(), &|| false).unwrap()
}

#[test]
fn reviewed_expression_and_noop_apply_have_query_receipts_without_record_counts() {
    let [before, after] = storage_pair();
    for replacement in ["42", "43"] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("query.accdb");
        let bundle = directory.path().join("journal");
        fs::write(&path, &before).unwrap();
        let staged = open(&path)
            .stage_reviewed_query_expression_with_budgets(
                review(&before, replacement),
                &mut budget(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(staged.query(), "Saved");
        assert_eq!(staged.native_id(), -2147483525);
        assert_eq!(staged.changed(), replacement == "43");
        assert_staged_read_state(&path, &before);
        assert!(!bundle.exists());
        let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
        assert_eq!(receipt.kind(), CompletionKind::Applied);
        assert_eq!(
            (receipt.matched_rows(), receipt.changed_rows()),
            (None, None)
        );
        assert_eq!(receipt.schema_change(), None);
        assert_eq!(
            receipt.query_change(),
            (replacement == "43").then_some(QueryChange::ExpressionSaved)
        );
        assert_completion(&receipt);
        session.release().unwrap();
        assert_eq!(
            fs::read(&path).unwrap(),
            if replacement == "43" { &after } else { &before }.as_slice()
        );
    }
}

#[test]
fn drift_cancellation_and_independent_quota_failures_retain_owner_without_journal() {
    let [before, _] = storage_pair();
    for case in 0..4 {
        let reviewed = review(&before, "43");
        let mut current = before.clone();
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
            .stage_reviewed_query_expression_with_budgets(
                reviewed,
                &mut native,
                &mut proof,
                &|| case == 3,
            )
            .err()
            .expect("source or quota must refuse before namespace creation");
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
    committed: bool,
    retired: bool,
) -> OfflineWriteSession {
    fs::write(path, before).unwrap();
    let plan = review(before, "43");
    let raw = plan.intent().encode(&mut budget(), &|| false).unwrap();
    assert_eq!(&raw[..8], b"TCQE0001");
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
    owner
}

fn retired_bundle(directory: &Path) -> PathBuf {
    let entries = fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(".tablecraft-retired-"))
        })
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), 1);
    entries[0].clone()
}

#[test]
fn cold_and_retained_recovery_and_settled_confirmation_keep_query_facts_distinct() {
    let [before, after] = storage_pair();
    for committed in [false, true] {
        for (retired, cold) in [(false, false), (false, true), (true, true)] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("recover.accdb");
            let bundle = directory.path().join("journal");
            let owner = retained_pair(&path, &bundle, &before, committed, retired);
            let (session, receipt) = if retired {
                owner.release().unwrap();
                open(&path)
                    .confirm_retired(&retired_bundle(directory.path()), &mut budget(), &|| false)
                    .unwrap()
            } else if cold {
                owner.release().unwrap();
                open(&path)
                    .recover_update(&bundle, &mut budget(), &|| false)
                    .unwrap()
            } else {
                owner.recover_pending(&mut budget(), &|| false).unwrap()
            };
            assert_eq!(receipt.schema_change(), None);
            assert_eq!(
                (receipt.matched_rows(), receipt.changed_rows()),
                (None, None)
            );
            assert_eq!(
                receipt.query_change(),
                committed.then_some(QueryChange::ExpressionSaved)
            );
            if retired {
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
            assert_completion(&receipt);
            session.release().unwrap();
            assert_eq!(
                fs::read(&path).unwrap(),
                if committed { &after } else { &before }.as_slice()
            );
        }
    }
}

#[test]
fn tcqe_decode_failure_is_not_retried_as_row_or_schema_intent() {
    let [before, _] = storage_pair();
    let mut raw = review(&before, "43")
        .intent()
        .encode(&mut budget(), &|| false)
        .unwrap();
    raw[7] = b'2';
    assert!(matches!(
        MutationIntent::decode(&raw, &mut budget(), &|| false),
        Err(Cause::QueryExpressionIntent(
            tablecraft_ace::QueryExpressionIntentError::UnsupportedVersion
        ))
    ));
}

#[test]
fn a_valid_io_journal_with_patched_after_is_retained_without_query_authority() {
    let [before, mut after] = storage_pair();
    *after.last_mut().unwrap() ^= 1;
    let raw = review(&before, "43")
        .intent()
        .encode(&mut budget(), &|| false)
        .unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("forged.accdb");
    let bundle = directory.path().join("journal");
    fs::write(&path, &before).unwrap();
    let mut owner = open(&path);
    let slot = owner.slot_mut().unwrap();
    let mut pair = OfflineJournalPair::create(
        slot.target.take().unwrap(),
        &bundle,
        raw,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    {
        let (mut target, mut journal) = pair.storage().unwrap();
        drop(
            tablecraft_io::journal::prepare_expected(
                &mut target,
                &mut journal,
                &before,
                &after,
                &mut budget(),
                &|| false,
            )
            .unwrap(),
        );
    }
    slot.pair = Some(pair);
    slot.state = SessionState::RetainedJournal;
    owner.release().unwrap();
    let failure = open(&path)
        .recover_update(&bundle, &mut budget(), &|| false)
        .err()
        .expect("patched AFTER must not qualify");
    assert_eq!(failure.phase(), Phase::Recovery);
    assert!(matches!(
        failure.cause(),
        Cause::QueryExpressionQualification(_)
    ));
    assert_eq!(failure.current_outcome(), None);
    assert!(failure.pair_state().is_some());
    failure.into_session().release().unwrap();
    assert_eq!(fs::read(&path).unwrap(), before);
    assert!(bundle.exists());
}

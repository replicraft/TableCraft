use super::*;
use crate::{
    SharedFile,
    journal::{RecoveryOutcome, prepare_expected},
};
use std::{
    cell::Cell,
    fs::{self, OpenOptions},
    io::{ErrorKind, Read, Seek, SeekFrom, Write},
    os::unix::fs::OpenOptionsExt,
    path::PathBuf,
};
use tablecraft_core::limits::ResourceBudget;

const BEFORE: &[u8] = b"before";
const AFTER: &[u8] = b"after!";

fn budget() -> ResourceBudget {
    ResourceBudget::new(10_000_000, 0, 10_000_000)
}

fn fresh_pair() -> (tempfile::TempDir, PathBuf, OfflineJournalPair) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("target");
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .unwrap();
    file.write_all(BEFORE).unwrap();
    drop(file);
    let target = OfflineTarget::open(&path, &mut budget(), &|| false).unwrap();
    let pair = OfflineJournalPair::create(
        target,
        &directory.path().join("active"),
        b"resolved-intent".to_vec(),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    (directory, path, pair)
}

fn complete(pair: &mut OfflineJournalPair, commit: bool) -> RecoveryOutcome {
    let (mut target, mut journal) = pair.storage().unwrap();
    let prepared = prepare_expected(
        &mut target,
        &mut journal,
        BEFORE,
        AFTER,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    if commit {
        prepared.commit(&mut budget(), &|| false).unwrap();
        RecoveryOutcome::Committed
    } else {
        prepared.rollback(&mut budget(), &|| false).unwrap()
    }
}

fn clean_pair(
    commit: bool,
) -> (
    tempfile::TempDir,
    PathBuf,
    OfflineJournalPair,
    RecoveryOutcome,
) {
    let (directory, path, mut pair) = fresh_pair();
    let outcome = complete(&mut pair, commit);
    pair.retire(outcome).unwrap();
    pair.cleanup_retired().unwrap();
    assert_eq!(pair.state(), PairState::RetiredClean);
    (directory, path, pair, outcome)
}

#[test]
fn clean_transfer_retains_same_target_and_continuous_exclusion_for_both_outcomes() {
    for committed in [false, true] {
        let (directory, path, pair, outcome) = clean_pair(committed);
        let identity = pair.target.identity();
        assert_eq!(pair.known_outcome(), Some(outcome));
        assert!(matches!(SharedFile::open(&path), Err(IoError::LockBusy)));
        let mut target = pair.into_target_after_clean().unwrap();
        assert!(target.identity() == identity);
        assert_eq!(target.lock_state(), TargetLockState::Held);
        assert!(matches!(SharedFile::open(&path), Err(IoError::LockBusy)));
        target.seek(SeekFrom::Start(0)).unwrap();
        let mut bytes = Vec::new();
        target.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, if committed { AFTER } else { BEFORE });
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        target.release().unwrap();
        SharedFile::open(&path).unwrap().close().unwrap();
    }
}

#[test]
fn transfer_does_not_reopen_a_retargeted_path_after_clean_retirement() {
    let (directory, path, pair, _) = clean_pair(true);
    let identity = pair.target.identity();
    let retained_name = directory.path().join("retained");
    fs::rename(&path, &retained_name).unwrap();
    fs::write(&path, b"other!").unwrap();
    let mut target = pair.into_target_after_clean().unwrap();
    assert!(target.identity() == identity);
    target.seek(SeekFrom::Start(0)).unwrap();
    let mut bytes = Vec::new();
    target.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, AFTER);
    assert!(matches!(
        SharedFile::open(&retained_name),
        Err(IoError::LockBusy)
    ));
    assert_eq!(fs::read(&path).unwrap(), b"other!");
    target.release().unwrap();
}

#[test]
fn active_unsettled_and_unclean_pairs_refuse_without_releasing_any_lock() {
    for phase in 0..4 {
        let (_directory, path, mut pair) = fresh_pair();
        match phase {
            0 => {}
            1 => {
                let _ = pair.storage().unwrap();
            }
            2 => {
                let outcome = complete(&mut pair, true);
                pair.note_outcome(outcome).unwrap();
            }
            3 => {
                let outcome = complete(&mut pair, false);
                pair.retire(outcome).unwrap();
            }
            _ => unreachable!(),
        }
        let state = pair.state();
        let known = pair.known_outcome();
        let calls = Cell::new(0);
        let failure = pair
            .transfer_with(|journal| {
                calls.set(calls.get() + 1);
                journal.try_release()
            })
            .err()
            .expect("lifecycle refusal");
        assert!(matches!(failure.cause(), PairError::InvalidState));
        assert_eq!(calls.get(), 0);
        assert_eq!(failure.pair().state(), state);
        assert_eq!(failure.pair().known_outcome(), known);
        assert!(matches!(SharedFile::open(&path), Err(IoError::LockBusy)));
        let (_, mut retained) = failure.into_parts();
        retained.release().unwrap();
        SharedFile::open(&path).unwrap().close().unwrap();
    }
}

#[test]
fn journal_unlock_failure_preserves_outcome_and_denies_transfer_retry() {
    for committed in [false, true] {
        for unlock_took_effect in [false, true] {
            let (_directory, path, pair, outcome) = clean_pair(committed);
            let calls = Cell::new(0);
            let failure = pair
                .transfer_with(|journal| {
                    calls.set(calls.get() + 1);
                    if unlock_took_effect {
                        journal.try_release().unwrap();
                    }
                    Err(IoError::Io(ErrorKind::Other))
                })
                .err()
                .expect("uncertain journal unlock");
            assert_eq!(calls.get(), 1);
            assert!(matches!(
                failure.cause(),
                PairError::Io(IoError::Io(ErrorKind::Other))
            ));
            assert_eq!(failure.pair().state(), PairState::ReleaseUncertain);
            assert_eq!(failure.pair().namespace_state(), PairState::RetiredClean);
            assert_eq!(failure.pair().known_outcome(), Some(outcome));
            assert!(matches!(SharedFile::open(&path), Err(IoError::LockBusy)));
            let (_, mut retained) = failure.into_parts();
            assert!(retained.storage().is_err());
            assert!(retained.release().is_err());
            let failure = retained
                .transfer_with(|_| {
                    calls.set(calls.get() + 1);
                    Ok(())
                })
                .err()
                .expect("no retry after uncertain release");
            assert_eq!(calls.get(), 1);
            assert_eq!(failure.pair().known_outcome(), Some(outcome));
        }
    }
}

#[test]
fn intact_retirement_transfer_preserves_artifact_and_same_held_target_for_both_outcomes() {
    for committed in [false, true] {
        let (directory, path, mut pair) = fresh_pair();
        assert_eq!(
            pair.cleanup_policy(),
            super::super::RetiredCleanupPolicy::RemoveWithBarrier
        );
        assert_eq!(pair.retirement_guarantee(), None);
        let outcome = complete(&mut pair, committed);
        pair.retire(outcome).unwrap();
        assert_eq!(pair.state(), PairState::Retired);
        assert_eq!(
            pair.retirement_guarantee(),
            Some(super::super::RetirementGuarantee::CapturedParentBarrier)
        );
        let identity = pair.target.identity();
        let artifact = fs::read_dir(directory.path())
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|entry| entry.is_dir())
            .unwrap();
        let mut entries: Vec<_> = fs::read_dir(&artifact)
            .unwrap()
            .map(|entry| {
                let path = entry.unwrap().path();
                (path.clone(), fs::read(path).unwrap())
            })
            .collect();
        entries.sort_by(|left, right| left.0.cmp(&right.0));
        assert_eq!(entries.len(), 3);
        let mut target = pair.into_target_after_retirement().unwrap();
        assert!(target.identity() == identity);
        assert_eq!(target.lock_state(), TargetLockState::Held);
        assert!(matches!(SharedFile::open(&path), Err(IoError::LockBusy)));
        target.seek(SeekFrom::Start(0)).unwrap();
        let mut bytes = Vec::new();
        target.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, if committed { AFTER } else { BEFORE });
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 2);
        assert_eq!(fs::read_dir(&artifact).unwrap().count(), 3);
        for (path, original) in entries {
            assert_eq!(fs::read(path).unwrap(), original);
        }
        target.release().unwrap();
        SharedFile::open(&path).unwrap().close().unwrap();
    }
}

#[test]
fn intact_transfer_refuses_nonretired_unknown_and_unheld_pairs_without_unlock_callback() {
    for phase in 0..6 {
        let (_directory, path, mut pair) = fresh_pair();
        match phase {
            0 => {}
            1 => {
                let _ = pair.storage().unwrap();
            }
            2 => {
                let outcome = complete(&mut pair, true);
                pair.note_outcome(outcome).unwrap();
            }
            3 => {
                let outcome = complete(&mut pair, true);
                pair.retire(outcome).unwrap();
                pair.cleanup_retired().unwrap();
            }
            4 => {
                let outcome = complete(&mut pair, true);
                pair.retire(outcome).unwrap();
                pair.known = None;
            }
            5 => {
                let outcome = complete(&mut pair, true);
                pair.retire(outcome).unwrap();
                pair.target.release().unwrap();
            }
            _ => unreachable!(),
        }
        let phase_before = pair.state();
        let known_before = pair.known_outcome();
        let calls = Cell::new(0);
        let failure = pair
            .transfer_retired_with(|_| {
                calls.set(calls.get() + 1);
                Ok(())
            })
            .err()
            .expect("only intact acknowledged retirement may transfer");
        assert!(matches!(failure.cause(), PairError::InvalidState));
        assert_eq!(calls.get(), 0);
        assert_eq!(failure.pair().state(), phase_before);
        assert_eq!(failure.pair().known_outcome(), known_before);
        if phase != 5 {
            assert!(matches!(SharedFile::open(&path), Err(IoError::LockBusy)));
        }
    }
}

#[test]
fn intact_transfer_journal_unlock_failure_retains_owner_and_forbids_retry() {
    let (directory, path, mut pair) = fresh_pair();
    let outcome = complete(&mut pair, true);
    pair.retire(outcome).unwrap();
    let failure = pair
        .transfer_retired_with(|_| Err(IoError::Io(ErrorKind::Other)))
        .err()
        .expect("uncertain journal unlock retains pair");
    assert_eq!(failure.pair().state(), PairState::ReleaseUncertain);
    assert_eq!(failure.pair().namespace_state(), PairState::Retired);
    assert_eq!(failure.pair().known_outcome(), Some(outcome));
    assert!(matches!(SharedFile::open(&path), Err(IoError::LockBusy)));
    let (_, pair) = failure.into_parts();
    let failure = pair
        .transfer_retired_with(|_| panic!("unlock must not be retried"))
        .err()
        .expect("uncertain release cannot transfer");
    assert_eq!(failure.pair().known_outcome(), Some(outcome));
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 2);
}

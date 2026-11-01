//! Real retained filesystem pairs consume V2 without reopening target paths.
use crate::{
    IoError, SharedFile,
    journal::{
        RecoveryOutcome,
        filesystem::{OfflineJournalPair, OfflineTarget, PairError},
        inspect_v2, prepare_resizable_expected,
    },
};
use std::{
    fs::{self, OpenOptions},
    io::{ErrorKind, Write},
    os::unix::fs::OpenOptionsExt,
    path::PathBuf,
};
use tablecraft_core::limits::ResourceBudget;

fn budget() -> ResourceBudget {
    ResourceBudget::new(10_000_000, 0, 10_000_000)
}

fn pair(before: &[u8]) -> (tempfile::TempDir, PathBuf, OfflineJournalPair) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("target");
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .unwrap();
    file.write_all(before).unwrap();
    drop(file);
    let target = OfflineTarget::open(&path, &mut budget(), &|| false).unwrap();
    let pair = OfflineJournalPair::create(
        target,
        &directory.path().join("active"),
        b"original-resolved-intent".to_vec(),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    (directory, path, pair)
}

#[test]
fn paired_v2_commit_and_rollback_retain_target_lease_through_cleanup() {
    for (before, after) in [
        (b"old".as_slice(), b"replacement".as_slice()),
        (b"before-tail".as_slice(), b"new".as_slice()),
    ] {
        for commit in [false, true] {
            let (directory, path, mut pair) = pair(before);
            assert!(matches!(SharedFile::open(&path), Err(IoError::LockBusy)));
            let outcome = {
                let (mut target, mut journal) = pair.resizable_storage().unwrap();
                let prepared = prepare_resizable_expected(
                    &mut target,
                    &mut journal,
                    before,
                    after,
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
            };
            assert_eq!(
                fs::read(&path).unwrap(),
                if commit { after } else { before }
            );
            assert!(matches!(SharedFile::open(&path), Err(IoError::LockBusy)));
            pair.retire(outcome).unwrap();
            pair.cleanup_retired().unwrap();
            assert!(matches!(
                pair.resizable_storage(),
                Err(PairError::InvalidState)
            ));
            let mut target = pair.into_target_after_clean().unwrap();
            assert!(matches!(SharedFile::open(&path), Err(IoError::LockBusy)));
            assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
            target.release().unwrap();
            SharedFile::open(&path).unwrap().close().unwrap();
        }
    }
}

#[test]
fn paired_v2_uses_retained_descriptor_when_original_path_is_retargeted() {
    let (directory, path, mut pair) = pair(b"old");
    let retained = directory.path().join("retained");
    fs::rename(&path, &retained).unwrap();
    fs::write(&path, b"other").unwrap();
    {
        let (mut target, mut journal) = pair.resizable_storage().unwrap();
        prepare_resizable_expected(
            &mut target,
            &mut journal,
            b"old",
            b"replacement",
            &mut budget(),
            &|| false,
        )
        .unwrap()
        .commit(&mut budget(), &|| false)
        .unwrap();
    }
    assert_eq!(fs::read(&retained).unwrap(), b"replacement");
    assert_eq!(fs::read(&path).unwrap(), b"other");
    assert!(matches!(
        SharedFile::open(&retained),
        Err(IoError::LockBusy)
    ));
    pair.release().unwrap();
}

#[test]
fn retired_v2_confirmation_cannot_issue_resize_or_write_capabilities() {
    let (directory, path, mut pair) = pair(b"old");
    {
        let (mut target, mut journal) = pair.resizable_storage().unwrap();
        prepare_resizable_expected(
            &mut target,
            &mut journal,
            b"old",
            b"replacement",
            &mut budget(),
            &|| false,
        )
        .unwrap()
        .commit(&mut budget(), &|| false)
        .unwrap();
    }
    pair.retire(RecoveryOutcome::Committed).unwrap();
    // The actual retirement name is owned by the pair's deterministic protocol.
    // Locate the intact retired directory, rather than assuming a suffix.
    let retired = fs::read_dir(directory.path())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|entry| entry.is_dir())
        .unwrap();
    pair.release().unwrap();
    drop(pair);
    let target = OfflineTarget::open(&path, &mut budget(), &|| false).unwrap();
    let mut pair =
        OfflineJournalPair::open_retired(target, &retired, 1024, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        pair.resizable_storage(),
        Err(PairError::InvalidState)
    ));
    let outcome = {
        let (mut target, mut journal) = pair.confirmation_storage().unwrap();
        assert_eq!(
            target.write(b"bad").unwrap_err().kind(),
            ErrorKind::PermissionDenied
        );
        assert_eq!(
            journal.write(b"bad").unwrap_err().kind(),
            ErrorKind::PermissionDenied
        );
        let checked = inspect_v2(&mut journal, &mut budget(), &|| false).unwrap();
        checked
            .confirm_completed(&mut target, &mut budget(), &|| false)
            .unwrap()
    };
    assert_eq!(outcome, RecoveryOutcome::Committed);
    assert_eq!(fs::read(&path).unwrap(), b"replacement");
    pair.acknowledge_retired(outcome).unwrap();
    pair.cleanup_retired().unwrap();
    let mut target = pair.into_target_after_clean().unwrap();
    target.release().unwrap();
}

mod failures;

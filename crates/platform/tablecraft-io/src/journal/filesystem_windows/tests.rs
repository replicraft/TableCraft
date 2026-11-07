//! Native Windows/local-NTFS lifecycle checks. Cross compilation is not runtime
//! evidence for these ACL, sharing, handle-relative rename or durability calls.
use super::*;
use crate::{
    IoError,
    journal::{RecoveryOutcome, inspect_v2, prepare_expected, prepare_resizable_expected, recover},
};
use std::{
    fs,
    io::{ErrorKind, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};
use tablecraft_core::limits::ResourceBudget;

const BEFORE: &[u8] = b"before";
const AFTER: &[u8] = b"after!";
const INTENT: &[u8] = b"original closed action intent";

fn budget() -> ResourceBudget {
    ResourceBudget::new(10_000_000, 0, 10_000_000)
}

fn create(directory: &Path, before: &[u8]) -> (PathBuf, PathBuf, OfflineJournalPair, FileIdentity) {
    let target_path = directory.join("target.accdb");
    let active = directory.join("active-journal");
    fs::write(&target_path, before).unwrap();
    let target = OfflineTarget::open(&target_path, &mut budget(), &|| false).unwrap();
    let identity = target.identity();
    let pair =
        OfflineJournalPair::create(target, &active, INTENT.to_vec(), &mut budget(), &|| false)
            .unwrap();
    (target_path, active, pair, identity)
}

fn retired_path(directory: &Path) -> PathBuf {
    let entries = fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with(".tablecraft-retired-")
        })
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), 1);
    entries.into_iter().next().unwrap()
}

fn target_bytes(target: &mut OfflineTarget) -> Vec<u8> {
    target.seek(SeekFrom::Start(0)).unwrap();
    let mut bytes = Vec::new();
    target.read_to_end(&mut bytes).unwrap();
    bytes
}

#[test]
fn windows_pair_v1_commit_retains_private_intact_artifact_and_same_held_target() {
    let directory = tempfile::tempdir().unwrap();
    let (path, active, mut pair, identity) = create(directory.path(), BEFORE);
    assert_eq!(pair.intent_bytes(), INTENT);
    {
        let (mut target, mut journal) = pair.storage().unwrap();
        prepare_expected(
            &mut target,
            &mut journal,
            BEFORE,
            AFTER,
            &mut budget(),
            &|| false,
        )
        .unwrap()
        .commit(&mut budget(), &|| false)
        .unwrap();
    }
    pair.retire(RecoveryOutcome::Committed).unwrap();
    assert_eq!(pair.state(), PairState::Retired);
    assert_eq!(pair.cleanup_policy(), RetiredCleanupPolicy::RetainIntact);
    assert_eq!(
        pair.retirement_guarantee(),
        Some(RetirementGuarantee::NtfsWriteThroughRename)
    );
    assert!(pair.cleanup_retired().is_err());
    let failed = pair.into_target_after_clean().err().unwrap();
    assert!(matches!(failed.cause(), PairError::InvalidState));
    assert_eq!(failed.pair().state(), PairState::Retired);
    let (_, pair) = failed.into_parts();
    let mut target = pair.into_target_after_retirement().unwrap();
    assert_eq!(target.lock_state(), TargetLockState::Held);
    assert!(target.identity() == identity);
    assert_eq!(target_bytes(&mut target), AFTER);
    assert!(fs::File::open(&path).is_err());
    let retired = retired_path(directory.path());
    assert!(!active.exists());
    let artifact = fs::read(&retired).unwrap();
    assert_eq!(&artifact[..8], b"TCJWIN01");
    assert_eq!(&artifact[224..224 + INTENT.len()], INTENT);
    // A successful strict reopen establishes that the retained artifact still
    // has the explicit protected current-user ACL, not inherited default access.
    let mut location =
        crate::windows_fs::Location::capture(&retired, &mut budget(), &|| false).unwrap();
    let mut private = Some(
        crate::windows_fs::open_private_write_through(&location.parent, &mut location.name)
            .unwrap(),
    );
    crate::windows_fs::close_file(&mut private).unwrap();
    assert_eq!(fs::read(&retired).unwrap(), artifact);
    target.release().unwrap();
    assert_eq!(fs::read(&path).unwrap(), AFTER);
    assert_eq!(fs::read(&retired).unwrap(), artifact);
}

#[test]
fn windows_pair_v2_resize_and_reserved_retired_confirmation_never_rewrite_artifact() {
    let directory = tempfile::tempdir().unwrap();
    let replacement = b"a genuinely longer replacement";
    let (path, _, mut pair, identity) = create(directory.path(), BEFORE);
    {
        let (mut target, mut journal) = pair.resizable_storage().unwrap();
        prepare_resizable_expected(
            &mut target,
            &mut journal,
            BEFORE,
            replacement,
            &mut budget(),
            &|| false,
        )
        .unwrap()
        .commit(&mut budget(), &|| false)
        .unwrap();
    }
    pair.retire(RecoveryOutcome::Committed).unwrap();
    let target = pair.into_target_after_retirement().unwrap();
    let retired = retired_path(directory.path());
    let artifact = fs::read(&retired).unwrap();
    let nonce = artifact[16..32]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(
        retired.file_name().unwrap().to_str().unwrap(),
        format!(".tablecraft-retired-{nonce}")
    );
    let mut pair = OfflineJournalPair::open_retired(
        target,
        &retired,
        INTENT.len() as u64,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(pair.state(), PairState::RetiredVerificationRequired);
    assert_eq!(pair.intent_bytes(), INTENT);
    {
        let (mut target, mut journal) = pair.confirmation_storage().unwrap();
        assert!(target.write(b"denied").is_err());
        assert!(journal.write(b"denied").is_err());
        let checked = inspect_v2(&mut journal, &mut budget(), &|| false).unwrap();
        assert_eq!(checked.before_bytes(), BEFORE);
        assert_eq!(checked.after_bytes(), replacement);
        assert_eq!(
            checked
                .confirm_completed(&mut target, &mut budget(), &|| false)
                .unwrap(),
            RecoveryOutcome::Committed
        );
    }
    pair.acknowledge_retired(RecoveryOutcome::Committed)
        .unwrap();
    let mut target = pair.into_target_after_retirement().unwrap();
    assert!(target.identity() == identity);
    assert_eq!(target_bytes(&mut target), replacement);
    assert_eq!(fs::read(&retired).unwrap(), artifact);
    target.release().unwrap();
    assert_eq!(fs::read(&path).unwrap(), replacement);
}

#[test]
fn windows_pair_active_pending_reopen_restores_exact_before_and_original_intent() {
    let directory = tempfile::tempdir().unwrap();
    let (path, active, mut pair, identity) = create(directory.path(), BEFORE);
    {
        let (mut target, mut journal) = pair.storage().unwrap();
        {
            let _prepared = prepare_expected(
                &mut target,
                &mut journal,
                BEFORE,
                AFTER,
                &mut budget(),
                &|| false,
            )
            .unwrap();
        }
        // Model a persisted interrupted prefix through the actual retained
        // target device; the complete pending journal was already synced.
        target.seek(SeekFrom::Start(0)).unwrap();
        target.write_all(&AFTER[..3]).unwrap();
    }
    pair.release().unwrap();
    let frozen_journal = fs::read(&active).unwrap();
    let target = OfflineTarget::open(&path, &mut budget(), &|| false).unwrap();
    assert!(target.identity() == identity);
    let mut reopened = OfflineJournalPair::open_existing(
        target,
        &active,
        INTENT.len() as u64,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(reopened.intent_bytes(), INTENT);
    let outcome = {
        let (mut target, mut journal) = reopened.storage().unwrap();
        recover(&mut target, &mut journal, &mut budget(), &|| false).unwrap()
    };
    assert_eq!(outcome, RecoveryOutcome::RolledBack);
    reopened.retire(outcome).unwrap();
    let mut target = reopened.into_target_after_retirement().unwrap();
    assert_eq!(target_bytes(&mut target), BEFORE);
    assert_eq!(
        fs::read(retired_path(directory.path())).unwrap(),
        frozen_journal
    );
    target.release().unwrap();
    assert_eq!(fs::read(&path).unwrap(), BEFORE);
}

#[test]
fn windows_pair_selected_wrong_target_or_nonprivate_sidecar_refuses_without_target_writes() {
    let directory = tempfile::tempdir().unwrap();
    let (original, active, mut pair, _) = create(directory.path(), BEFORE);
    pair.release().unwrap();
    let frozen = fs::read(&active).unwrap();
    let other = directory.path().join("other.accdb");
    fs::write(&other, BEFORE).unwrap();
    let target = OfflineTarget::open(&other, &mut budget(), &|| false).unwrap();
    let failed = OfflineJournalPair::open_existing(
        target,
        &active,
        INTENT.len() as u64,
        &mut budget(),
        &|| false,
    )
    .err()
    .unwrap();
    assert!(matches!(
        failed.cause(),
        PairError::Binding(BindingError::ObjectMismatch)
    ));
    let mut target = failed.into_target();
    assert_eq!(target_bytes(&mut target), BEFORE);
    target.release().unwrap();
    assert_eq!(fs::read(&original).unwrap(), BEFORE);
    assert_eq!(fs::read(&active).unwrap(), frozen);

    let ordinary = directory.path().join("nonprivate-journal");
    fs::write(&ordinary, &frozen).unwrap();
    let target = OfflineTarget::open(&original, &mut budget(), &|| false).unwrap();
    let failed = OfflineJournalPair::open_existing(
        target,
        &ordinary,
        INTENT.len() as u64,
        &mut budget(),
        &|| false,
    )
    .err()
    .unwrap();
    assert!(matches!(
        failed.cause(),
        PairError::Io(IoError::Io(ErrorKind::PermissionDenied))
    ));
    let mut target = failed.into_target();
    assert_eq!(target_bytes(&mut target), BEFORE);
    target.release().unwrap();
    assert_eq!(fs::read(&ordinary).unwrap(), frozen);
    assert_eq!(fs::read(&original).unwrap(), BEFORE);
}

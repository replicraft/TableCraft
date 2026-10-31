use super::{NamespaceOps, RenameFailure, TerminalPermit, cleanup_with, retire_with};
use crate::{
    IoError,
    journal::filesystem::{OfflineJournalPair, OfflineTarget, PairState},
    journal::{RecoveryOutcome, inspect_v1, prepare_expected},
};
use std::{
    fs::{self, OpenOptions},
    io::{ErrorKind, Write},
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};
use tablecraft_core::limits::{Resource, ResourceBudget};

fn budget() -> ResourceBudget {
    ResourceBudget::new(10_000_000, 0, 10_000_000)
}

struct Model {
    events: Vec<&'static str>,
    rename: u8,
    barrier_error: bool,
    cleanup_error: bool,
}

impl NamespaceOps for Model {
    fn verify_active(&mut self) -> Result<(), IoError> {
        self.events.push("verify");
        Ok(())
    }

    fn rename(&mut self) -> Result<(), RenameFailure> {
        self.events.push("rename");
        match self.rename {
            1 => Err(RenameFailure::Collision(IoError::Io(
                ErrorKind::AlreadyExists,
            ))),
            2 | 3 => Err(RenameFailure::Uncertain(IoError::Io(ErrorKind::Other))),
            _ => Ok(()),
        }
    }

    fn sync_parent(&mut self) -> Result<(), IoError> {
        self.events.push("barrier");
        if self.barrier_error {
            return Err(IoError::Io(ErrorKind::Other));
        }
        Ok(())
    }

    fn cleanup(&mut self) -> Result<(), IoError> {
        self.events.push("cleanup");
        if self.cleanup_error {
            return Err(IoError::Io(ErrorKind::Other));
        }
        Ok(())
    }
}

#[test]
fn retirement_unknown_and_collision_never_retry_or_cleanup() {
    for rename in [1, 2, 3] {
        let mut operations = Model {
            events: Vec::new(),
            rename,
            barrier_error: false,
            cleanup_error: false,
        };
        let mut state = PairState::OutcomeRecorded;
        let mut permit = TerminalPermit::reserve(10, &mut budget(), &|| false).unwrap();
        let error = retire_with(
            &mut operations,
            &mut state,
            &mut permit,
            RecoveryOutcome::Committed,
        )
        .unwrap_err();
        assert_eq!(error.outcome, Some(RecoveryOutcome::Committed));
        assert_eq!(
            state,
            if rename == 1 {
                PairState::RetirementBlocked
            } else {
                PairState::RetirementUncertain
            }
        );
        assert_eq!(operations.events, ["verify", "rename"]);
        assert!(
            retire_with(
                &mut operations,
                &mut state,
                &mut permit,
                RecoveryOutcome::Committed
            )
            .is_err()
        );
        assert!(
            cleanup_with(
                &mut operations,
                &mut state,
                &mut permit,
                RecoveryOutcome::Committed
            )
            .is_err()
        );
        assert_eq!(operations.events, ["verify", "rename"]);
    }
}

#[test]
fn retirement_barrier_and_garbage_errors_preserve_known_outcome() {
    let mut operations = Model {
        events: Vec::new(),
        rename: 0,
        barrier_error: true,
        cleanup_error: false,
    };
    let mut state = PairState::OutcomeRecorded;
    let mut permit = TerminalPermit::reserve(10, &mut budget(), &|| false).unwrap();
    let error = retire_with(
        &mut operations,
        &mut state,
        &mut permit,
        RecoveryOutcome::RolledBack,
    )
    .unwrap_err();
    assert_eq!(state, PairState::RetirementUnconfirmed);
    assert_eq!(error.outcome, Some(RecoveryOutcome::RolledBack));
    assert_eq!(operations.events, ["verify", "rename", "barrier"]);
    assert!(
        cleanup_with(
            &mut operations,
            &mut state,
            &mut permit,
            RecoveryOutcome::RolledBack
        )
        .is_err()
    );

    let mut operations = Model {
        events: Vec::new(),
        rename: 0,
        barrier_error: false,
        cleanup_error: true,
    };
    let mut state = PairState::OutcomeRecorded;
    let mut permit = TerminalPermit::reserve(10, &mut budget(), &|| false).unwrap();
    retire_with(
        &mut operations,
        &mut state,
        &mut permit,
        RecoveryOutcome::Committed,
    )
    .unwrap();
    let error = cleanup_with(
        &mut operations,
        &mut state,
        &mut permit,
        RecoveryOutcome::Committed,
    )
    .unwrap_err();
    assert_eq!(state, PairState::RetiredGarbageUncertain);
    assert_eq!(error.outcome, Some(RecoveryOutcome::Committed));
    assert_eq!(
        operations.events,
        ["verify", "rename", "barrier", "cleanup"]
    );
    assert!(
        cleanup_with(
            &mut operations,
            &mut state,
            &mut permit,
            RecoveryOutcome::Committed
        )
        .is_err()
    );
    assert_eq!(
        operations.events,
        ["verify", "rename", "barrier", "cleanup"]
    );
}

fn fixture() -> (tempfile::TempDir, PathBuf, PathBuf, OfflineJournalPair) {
    let root = tempfile::tempdir().unwrap();
    let target = root.path().join("target");
    let bundle = root.path().join("active");
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(&target)
        .unwrap();
    file.write_all(b"original").unwrap();
    drop(file);
    let target_owner = OfflineTarget::open(&target, &mut budget(), &|| false).unwrap();
    let pair = OfflineJournalPair::create(
        target_owner,
        &bundle,
        b"typed-intent".to_vec(),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    (root, target, bundle, pair)
}

fn commit(pair: &mut OfflineJournalPair) {
    let (mut target, mut journal) = pair.storage().unwrap();
    prepare_expected(
        &mut target,
        &mut journal,
        b"original",
        b"modified",
        &mut budget(),
        &|| false,
    )
    .unwrap()
    .commit(&mut budget(), &|| false)
    .unwrap();
}

fn retired_path(root: &Path, active: &Path) -> PathBuf {
    let binding = fs::read(active.join("binding")).unwrap();
    let nonce: String = binding[16..32]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    root.join(format!(".tablecraft-retired-{nonce}"))
}

#[test]
fn paired_real_commit_retires_whole_bundle_then_cleans_and_releases() {
    let (root, target, bundle, mut pair) = fixture();
    let retired = retired_path(root.path(), &bundle);
    commit(&mut pair);
    pair.retire(RecoveryOutcome::Committed).unwrap();
    assert_eq!(pair.known_outcome(), Some(RecoveryOutcome::Committed));
    assert_eq!(pair.state(), PairState::Retired);
    assert!(!bundle.exists());
    assert!(retired.join("journal").exists());
    pair.cleanup_retired().unwrap();
    assert!(!retired.exists());
    pair.release().unwrap();
    assert_eq!(pair.namespace_state(), PairState::RetiredClean);
    assert_eq!(pair.known_outcome(), Some(RecoveryOutcome::Committed));
    assert_eq!(fs::read(&target).unwrap(), b"modified");
}

#[test]
fn paired_real_pending_reopen_recovers_without_original_sql_replay() {
    let (_root, target_path, bundle, mut pair) = fixture();
    {
        let (mut target, mut journal) = pair.storage().unwrap();
        drop(
            prepare_expected(
                &mut target,
                &mut journal,
                b"original",
                b"modified",
                &mut budget(),
                &|| false,
            )
            .unwrap(),
        );
    }
    pair.release().unwrap();
    drop(pair);
    let target = OfflineTarget::open(&target_path, &mut budget(), &|| false).unwrap();
    let mut pair =
        OfflineJournalPair::open_existing(target, &bundle, 1024, &mut budget(), &|| false).unwrap();
    let outcome = {
        let (mut target, mut journal) = pair.storage().unwrap();
        inspect_v1(&mut journal, &mut budget(), &|| false)
            .unwrap()
            .recover(&mut target, &mut budget(), &|| false)
            .unwrap()
    };
    assert_eq!(outcome, RecoveryOutcome::RolledBack);
    pair.retire(outcome).unwrap();
    pair.cleanup_retired().unwrap();
    pair.release().unwrap();
    assert_eq!(fs::read(target_path).unwrap(), b"original");
}

#[test]
fn paired_real_collision_and_foreign_garbage_preserve_targets_and_other_entries() {
    let (root, target, bundle, mut pair) = fixture();
    let retired = retired_path(root.path(), &bundle);
    fs::create_dir(&retired).unwrap();
    fs::write(retired.join("foreign"), b"keep").unwrap();
    commit(&mut pair);
    let error = pair.retire(RecoveryOutcome::Committed).unwrap_err();
    assert_eq!(error.outcome, Some(RecoveryOutcome::Committed));
    assert_eq!(pair.state(), PairState::RetirementBlocked);
    assert!(bundle.join("journal").exists());
    assert_eq!(fs::read(retired.join("foreign")).unwrap(), b"keep");
    assert_eq!(fs::read(target).unwrap(), b"modified");

    let (root, target, bundle, mut pair) = fixture();
    let retired = retired_path(root.path(), &bundle);
    commit(&mut pair);
    pair.retire(RecoveryOutcome::Committed).unwrap();
    fs::rename(retired.join("binding"), retired.join("original-binding")).unwrap();
    fs::write(retired.join("binding"), b"foreign").unwrap();
    let error = pair.cleanup_retired().unwrap_err();
    assert_eq!(error.outcome, Some(RecoveryOutcome::Committed));
    assert_eq!(pair.state(), PairState::RetiredGarbageUncertain);
    assert_eq!(fs::read(retired.join("binding")).unwrap(), b"foreign");
    assert!(retired.join("original-binding").exists());
    assert_eq!(fs::read(target).unwrap(), b"modified");
}

#[test]
fn paired_terminal_work_is_reserved_before_new_bundle_effects() {
    let mut observed = budget();
    let before = observed.remaining(Resource::Steps);
    let _permit = TerminalPermit::reserve(10, &mut observed, &|| false).unwrap();
    let used = before - observed.remaining(Resource::Steps);
    assert!(
        TerminalPermit::reserve(10, &mut ResourceBudget::new(0, 0, used - 1), &|| false).is_err()
    );
    assert!(TerminalPermit::reserve(10, &mut ResourceBudget::new(0, 0, used), &|| false).is_ok());
    let root = tempfile::tempdir().unwrap();
    let target_path = root.path().join("target");
    fs::write(&target_path, b"original").unwrap();
    let target = OfflineTarget::open(&target_path, &mut budget(), &|| false).unwrap();
    let bundle = root.path().join("active");
    let failed = OfflineJournalPair::create(
        target,
        &bundle,
        b"intent".to_vec(),
        &mut ResourceBudget::new(0, 0, 0),
        &|| false,
    )
    .err()
    .unwrap();
    assert!(!failed.incomplete_setup());
    assert!(!bundle.exists());
    assert_eq!(fs::read(target_path).unwrap(), b"original");
}

#[test]
fn retirement_confirmation_repeats_only_known_successful_rename_barrier() {
    let mut events = Vec::new();
    let mut state = PairState::RetirementUnconfirmed;
    super::confirm_barrier(
        &mut state,
        RecoveryOutcome::Committed,
        &mut budget(),
        &|| false,
        || {
            events.push("parent_barrier");
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(state, PairState::Retired);
    assert_eq!(events, ["parent_barrier"]);
    let mut state = PairState::RetirementUncertain;
    assert!(
        super::confirm_barrier(
            &mut state,
            RecoveryOutcome::Committed,
            &mut budget(),
            &|| false,
            || { panic!("unknown rename must not run a barrier or retry namespace work") }
        )
        .is_err()
    );
}

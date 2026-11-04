use super::{OfflineJournalPair, OfflineTarget, PairError, PairSetupFailure};
use crate::{IoError, SharedFile, journal::filesystem::TargetLockState};
use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};
use tablecraft_core::limits::ResourceBudget;

const ORIGINAL: &[u8] = b"unchanged target bytes";

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000, 0, 1_000_000)
}

fn must_fail(result: Result<OfflineJournalPair, PairSetupFailure>) -> PairSetupFailure {
    match result {
        Ok(_) => panic!("expected retained setup failure"),
        Err(failure) => failure,
    }
}

fn target(root: &Path) -> (PathBuf, OfflineTarget) {
    let path = root.join("target");
    fs::write(&path, ORIGINAL).unwrap();
    let target = OfflineTarget::open(&path, &mut budget(), &|| false).unwrap();
    (path, target)
}

// Stop the actual factory only after its actual journal lock is held. This
// produces a real partial setup owner, not fabricated private file descriptors.
fn partial(root: &Path) -> (PathBuf, PathBuf, PairSetupFailure) {
    let (path, target) = target(root);
    let bundle = root.join("active");
    let journal = bundle.join("journal");
    let failure = must_fail(OfflineJournalPair::create(
        target,
        &bundle,
        b"intent".to_vec(),
        &mut budget(),
        &|| {
            if !journal.exists() {
                return false;
            }
            match SharedFile::open(&journal) {
                Ok(file) => {
                    file.close().unwrap();
                    false
                }
                Err(IoError::LockBusy) => true,
                Err(error) => panic!("unexpected journal observation: {error}"),
            }
        },
    ));
    assert!(matches!(failure.cause(), PairError::Io(IoError::Cancelled)));
    assert!(
        failure
            .incomplete
            .iter()
            .any(|bundle| bundle.journal.is_some())
    );
    (path, bundle, failure)
}

fn contents(bundle: &Path) -> Vec<(String, Vec<u8>)> {
    let mut entries = fs::read_dir(bundle)
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            (
                path.file_name().unwrap().to_str().unwrap().to_owned(),
                fs::read(path).unwrap(),
            )
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    entries
}

#[test]
fn explicit_collision_release_preserves_foreign_namespace_and_original_diagnostics() {
    let root = tempfile::tempdir().unwrap();
    let (path, target) = target(root.path());
    let bundle = root.path().join("occupied");
    fs::create_dir(&bundle).unwrap();
    fs::write(bundle.join("sentinel"), b"foreign namespace").unwrap();
    let before = contents(&bundle);
    let mut failure = must_fail(OfflineJournalPair::create(
        target,
        &bundle,
        vec![],
        &mut budget(),
        &|| false,
    ));
    assert!(matches!(
        failure.cause(),
        PairError::Io(IoError::Io(ErrorKind::AlreadyExists))
    ));
    assert!(matches!(SharedFile::open(&path), Err(IoError::LockBusy)));

    failure.release_resources().unwrap();
    assert!(matches!(
        failure.cause(),
        PairError::Io(IoError::Io(ErrorKind::AlreadyExists))
    ));
    assert_eq!(failure.target.lock_state(), TargetLockState::Released);
    assert!(matches!(
        failure.release_resources(),
        Err(PairError::InvalidState)
    ));
    SharedFile::open(&path).unwrap().close().unwrap();
    assert_eq!(contents(&bundle), before);
    assert_eq!(fs::read(path).unwrap(), ORIGINAL);
}

#[test]
fn explicit_partial_setup_release_unlocks_all_handles_without_changing_any_files() {
    let root = tempfile::tempdir().unwrap();
    let (path, bundle, mut failure) = partial(root.path());
    let before = contents(&bundle);
    assert!(matches!(
        SharedFile::open(&bundle.join("journal")),
        Err(IoError::LockBusy)
    ));
    failure.release_resources().unwrap();
    assert!(matches!(failure.cause(), PairError::Io(IoError::Cancelled)));
    assert!(failure.incomplete_setup());
    SharedFile::open(&path).unwrap().close().unwrap();
    SharedFile::open(&bundle.join("journal"))
        .unwrap()
        .close()
        .unwrap();
    assert_eq!(contents(&bundle), before);
    assert_eq!(fs::read(path).unwrap(), ORIGINAL);
}

#[test]
fn journal_unlock_error_retains_diagnostics_and_denies_second_attempt_even_after_effect() {
    for effective in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let (path, bundle, mut failure) = partial(root.path());
        let before = contents(&bundle);
        let phase = failure.incomplete[0].phase;
        let result = failure.release_with(
            |journal| {
                if effective {
                    journal.try_release()?;
                }
                Err(IoError::Io(ErrorKind::PermissionDenied))
            },
            OfflineTarget::release,
        );
        assert!(matches!(
            result,
            Err(PairError::Io(IoError::Io(ErrorKind::PermissionDenied)))
        ));
        assert!(matches!(failure.cause(), PairError::Io(IoError::Cancelled)));
        assert_eq!(failure.incomplete[0].phase, phase);
        assert_eq!(failure.target.lock_state(), TargetLockState::Released);
        assert!(matches!(
            failure.release_with(
                |_| panic!("second journal unlock"),
                |_| panic!("second target unlock")
            ),
            Err(PairError::InvalidState)
        ));
        assert_eq!(contents(&bundle), before);
        let mut target = failure.into_target();
        assert!(target.storage_mut().is_err());
        assert_eq!(fs::read(path).unwrap(), ORIGINAL);
    }
}

#[test]
fn target_release_error_does_not_attempt_journal_release_or_repeat_callbacks() {
    let root = tempfile::tempdir().unwrap();
    let (path, bundle, mut failure) = partial(root.path());
    let before = contents(&bundle);
    let result = failure.release_with(
        |_| panic!("journal must remain untouched"),
        |target| {
            target.release()?;
            Err(IoError::Io(ErrorKind::PermissionDenied))
        },
    );
    assert!(matches!(
        result,
        Err(PairError::Io(IoError::Io(ErrorKind::PermissionDenied)))
    ));
    assert!(matches!(
        failure.release_resources(),
        Err(PairError::InvalidState)
    ));
    assert!(matches!(failure.cause(), PairError::Io(IoError::Cancelled)));
    assert!(matches!(
        SharedFile::open(&bundle.join("journal")),
        Err(IoError::LockBusy)
    ));
    assert_eq!(contents(&bundle), before);
    assert_eq!(fs::read(path).unwrap(), ORIGINAL);
}

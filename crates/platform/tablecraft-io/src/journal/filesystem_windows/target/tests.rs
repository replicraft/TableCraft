//! Native Windows sharing/close tests; require a local NTFS temporary directory.
use super::*;
use std::fs;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000, 0, 1_000_000)
}

#[test]
fn windows_offline_target_holds_original_identity_and_actual_release_allows_reopen() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("Database1.accdb");
    let bytes = b"retained original database bytes";
    fs::write(&path, bytes).unwrap();
    let mut target = OfflineTarget::open(&path, &mut budget(), &|| false).unwrap();
    let identity = target.identity();
    let parent_identity = target.parent_identity();
    assert_eq!(target.lock_state(), TargetLockState::Held);
    assert_eq!(target.length().unwrap(), bytes.len() as u64);
    target.require_entry(&mut budget(), &|| false).unwrap();
    target.sync_baseline(&mut budget(), &|| false).unwrap();
    assert!(target.identity() == identity);
    assert!(target.parent_identity() == parent_identity);
    let mut read = Vec::new();
    target.read_to_end(&mut read).unwrap();
    assert_eq!(read, bytes);
    assert_eq!(
        OfflineTarget::open(&path, &mut budget(), &|| false).err(),
        Some(IoError::LockBusy)
    );
    assert!(fs::File::open(&path).is_err());

    target.release().unwrap();
    assert_eq!(target.lock_state(), TargetLockState::Released);
    assert!(target.length().is_err());
    assert!(target.read(&mut [0]).is_err());
    assert!(target.seek(SeekFrom::Start(0)).is_err());
    assert!(target.release().is_err());
    assert_eq!(fs::read(&path).unwrap(), bytes);
    let mut reopened = OfflineTarget::open(&path, &mut budget(), &|| false).unwrap();
    assert!(reopened.identity() == identity);
    assert!(reopened.parent_identity() == parent_identity);
    reopened.release().unwrap();
}

#[test]
fn windows_offline_target_existing_open_is_case_insensitive_and_rejects_directories() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("Database1.accdb");
    let alternative = directory.path().join("database1.ACCDB");
    fs::write(&path, b"original").unwrap();
    let mut target = OfflineTarget::open(&alternative, &mut budget(), &|| false).unwrap();
    let mut bytes = Vec::new();
    target.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"original");
    target.release().unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"original");
    let child_directory = directory.path().join("directory.accdb");
    fs::create_dir(&child_directory).unwrap();
    assert!(OfflineTarget::open(&child_directory, &mut budget(), &|| false).is_err());
    assert!(child_directory.is_dir());
}

#[test]
fn windows_offline_target_cancellation_does_not_modify_or_retain_a_file_lock() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("Database1.accdb");
    fs::write(&path, b"original").unwrap();
    assert_eq!(
        OfflineTarget::open(&path, &mut budget(), &|| true).err(),
        Some(IoError::Cancelled)
    );
    assert_eq!(fs::read(&path).unwrap(), b"original");
    let mut target = OfflineTarget::open(&path, &mut budget(), &|| false).unwrap();
    target.release().unwrap();
}

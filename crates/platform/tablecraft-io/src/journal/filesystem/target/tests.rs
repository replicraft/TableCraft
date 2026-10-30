use super::{OfflineTarget, TargetLockState};
use crate::{IoError, SharedFile, namespace::Location};
use std::{
    fs,
    io::{ErrorKind, Read, Seek, SeekFrom},
    os::unix::fs::symlink,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000, 0, 1_000_000)
}

#[test]
fn qualify_retained_target_original_bytes_and_exclusive_descriptor() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("target");
    fs::write(&path, b"original").unwrap();
    let mut target = OfflineTarget::open(&path, &mut budget(), &|| false).unwrap();
    assert_eq!(target.lock_state(), TargetLockState::Held);
    assert_eq!(target.length().unwrap(), 8);
    assert!(matches!(SharedFile::open(&path), Err(IoError::LockBusy)));

    let mut bytes = Vec::new();
    target.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"original");
    target.seek(SeekFrom::Start(0)).unwrap();
    target.sync_baseline(&mut budget(), &|| false).unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"original");

    target.release().unwrap();
    assert_eq!(target.lock_state(), TargetLockState::Released);
    assert!(target.storage_mut().is_err());
    assert!(target.read(&mut [0]).is_err());
    SharedFile::open(&path).unwrap().close().unwrap();
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
}

#[test]
fn target_capture_stays_in_original_parent_after_text_path_retargeting() {
    let root = tempfile::tempdir().unwrap();
    let a = root.path().join("a");
    let b = root.path().join("b");
    fs::create_dir(&a).unwrap();
    fs::create_dir(&b).unwrap();
    fs::write(a.join("target"), b"original-A").unwrap();
    fs::write(b.join("target"), b"other-B").unwrap();
    let selector = root.path().join("selected");
    symlink(&a, &selector).unwrap();
    let location = Location::capture(&selector.join("target"), &mut budget(), &|| false).unwrap();
    fs::remove_file(&selector).unwrap();
    symlink(&b, &selector).unwrap();

    let mut target = OfflineTarget::open_location(location, &mut budget(), &|| false).unwrap();
    let mut bytes = Vec::new();
    target.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"original-A");
    target.require_entry(&mut budget(), &|| false).unwrap();
    target.release().unwrap();
    assert_eq!(fs::read(a.join("target")).unwrap(), b"original-A");
    assert_eq!(fs::read(b.join("target")).unwrap(), b"other-B");
}

#[test]
fn target_final_symlink_refuses_but_explicit_hardlink_alias_retains_identity() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("target");
    let link = root.path().join("link");
    let alias = root.path().join("alias");
    fs::write(&path, b"original").unwrap();
    symlink(&path, &link).unwrap();
    fs::hard_link(&path, &alias).unwrap();
    assert!(OfflineTarget::open(&link, &mut budget(), &|| false).is_err());
    let mut original = OfflineTarget::open(&path, &mut budget(), &|| false).unwrap();
    let identity = original.identity();
    original.release().unwrap();
    let mut selected = OfflineTarget::open(&alias, &mut budget(), &|| false).unwrap();
    assert!(selected.identity() == identity);
    selected.release().unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"original");
}

#[test]
fn target_capture_pools_and_final_cancellation_do_not_write_or_retain_locks() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("target");
    fs::write(&path, b"original").unwrap();
    let mut measured = budget();
    let polls = std::cell::Cell::new(0);
    let mut target = OfflineTarget::open(&path, &mut measured, &|| {
        polls.set(polls.get() + 1);
        false
    })
    .unwrap();
    target.release().unwrap();
    let resources = [Resource::Bytes, Resource::Steps];
    let used =
        resources.map(|resource| budget().remaining(resource) - measured.remaining(resource));
    for (position, resource) in resources.into_iter().enumerate() {
        for exact in [false, true] {
            let mut limits = [1_000_000, 1_000_000];
            limits[position] = used[position] - u64::from(!exact);
            let mut allowance = ResourceBudget::new(limits[0], 0, limits[1]);
            let result = OfflineTarget::open(&path, &mut allowance, &|| false);
            assert_eq!(result.is_ok(), exact);
            if let Ok(mut target) = result {
                assert_eq!(allowance.remaining(resource), 0);
                target.release().unwrap();
            }
            SharedFile::open(&path).unwrap().close().unwrap();
        }
    }

    let calls = std::cell::Cell::new(0);
    let terminal = polls.get();
    assert!(
        OfflineTarget::open(&path, &mut budget(), &|| {
            calls.set(calls.get() + 1);
            calls.get() == terminal
        })
        .is_err()
    );
    SharedFile::open(&path).unwrap().close().unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"original");
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
}

#[test]
fn target_unlock_errors_keep_identity_but_never_claim_retained_exclusivity() {
    for after_effect in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("target");
        fs::write(&path, b"original").unwrap();
        let mut target = OfflineTarget::open(&path, &mut budget(), &|| false).unwrap();
        let identity = target.identity();
        let result = target.release_with(|file| {
            if after_effect {
                file.try_release()?;
            }
            Err(IoError::Io(ErrorKind::Other))
        });
        assert!(result.is_err());
        assert_eq!(target.lock_state(), TargetLockState::ReleaseUncertain);
        assert!(target.identity() == identity);
        assert!(target.storage_mut().is_err());
        assert!(target.read(&mut [0]).is_err());
        assert!(target.seek(SeekFrom::Start(0)).is_err());
        assert!(target.release().is_err());
        if after_effect {
            SharedFile::open(&path).unwrap().close().unwrap();
        } else {
            assert!(matches!(SharedFile::open(&path), Err(IoError::LockBusy)));
        }
        assert_eq!(fs::read(&path).unwrap(), b"original");
    }
}

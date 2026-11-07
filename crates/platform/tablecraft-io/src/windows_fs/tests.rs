//! Execute only on Windows with a local NTFS temporary directory. Cross builds
//! establish compilation only, not ACL, sharing or filesystem behavior.
use super::*;
use std::{
    fs,
    io::{Read, Seek, SeekFrom, Write},
};
use tablecraft_core::limits::{Resource, ResourceBudget};

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000, 0, 1_000_000)
}

fn admit_security(work: &mut ResourceBudget) {
    work.charge(Resource::Bytes, PRIVATE_SECURITY_BYTES)
        .unwrap();
    work.charge(Resource::Steps, PRIVATE_SECURITY_STEPS)
        .unwrap();
}

#[test]
fn windows_private_sidecar_security_survives_close_and_strict_reopen() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("private-journal");
    let mut work = budget();
    let mut location = Location::capture(&path, &mut work, &|| false).unwrap();
    let parent_id = location.identity.clone();
    admit_security(&mut work);
    let file = create_private_write_through(&location.parent, &mut location.name)
        .unwrap_or_else(|failure| panic!("private creation failed: {}", failure.cause));
    let original_id = identity(&file).unwrap();
    let mut retained = Some(file);
    retained
        .as_mut()
        .unwrap()
        .write_all(b"private journal data")
        .unwrap();
    flush(retained.as_ref().unwrap()).unwrap();
    assert!(fs::File::open(&path).is_err());
    assert!(identity(&location.parent).unwrap() == parent_id);
    close_file(&mut retained).unwrap();
    assert!(retained.is_none());

    admit_security(&mut work);
    let mut reopened =
        Some(open_private_write_through(&location.parent, &mut location.name).unwrap());
    assert!(identity(reopened.as_ref().unwrap()).unwrap() == original_id);
    assert!(identity(&location.parent).unwrap() == parent_id);
    let mut data = Vec::new();
    reopened.as_mut().unwrap().read_to_end(&mut data).unwrap();
    assert_eq!(data, b"private journal data");
    close_file(&mut reopened).unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"private journal data");
}

#[test]
fn windows_default_or_inherited_sidecar_dacl_does_not_qualify_as_explicit_private_security() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("default-security");
    let mut work = budget();
    let mut location = Location::capture(&path, &mut work, &|| false).unwrap();
    // Same default-security route as ordinary publication, deliberately not the
    // protected explicit descriptor used by private journal creation.
    let mut file = Some(
        create_exclusive_write_through(&location.parent, &mut location.name)
            .unwrap_or_else(|failure| panic!("default creation failed: {}", failure.cause)),
    );
    file.as_mut()
        .unwrap()
        .write_all(b"untouched default-security file")
        .unwrap();
    close_file(&mut file).unwrap();
    admit_security(&mut work);
    assert!(open_private_write_through(&location.parent, &mut location.name).is_err());
    assert_eq!(fs::read(&path).unwrap(), b"untouched default-security file");
}

#[test]
fn windows_private_sidecar_hard_link_aliases_are_refused_without_modification() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("private-journal");
    let alias = directory.path().join("alias");
    let mut work = budget();
    let mut location = Location::capture(&path, &mut work, &|| false).unwrap();
    admit_security(&mut work);
    let mut file = Some(
        create_private_write_through(&location.parent, &mut location.name)
            .unwrap_or_else(|failure| panic!("private creation failed: {}", failure.cause)),
    );
    file.as_mut().unwrap().write_all(b"original").unwrap();
    close_file(&mut file).unwrap();
    fs::hard_link(&path, &alias).unwrap();
    admit_security(&mut work);
    assert!(open_private_write_through(&location.parent, &mut location.name).is_err());
    assert_eq!(fs::read(&path).unwrap(), b"original");
    assert_eq!(fs::read(&alias).unwrap(), b"original");
}

#[test]
fn windows_existing_relative_file_open_never_creates_or_truncates() {
    let directory = tempfile::tempdir().unwrap();
    let missing = directory.path().join("missing.accdb");
    let mut work = budget();
    let mut location = Location::capture(&missing, &mut work, &|| false).unwrap();
    assert!(open_existing_write_through(&location.parent, &mut location.name).is_err());
    assert!(!missing.exists());
    fs::write(&missing, b"original bytes").unwrap();
    let mut file = Some(open_existing_write_through(&location.parent, &mut location.name).unwrap());
    assert_eq!(file.as_ref().unwrap().metadata().unwrap().len(), 14);
    file.as_mut().unwrap().seek(SeekFrom::Start(0)).unwrap();
    let mut data = Vec::new();
    file.as_mut().unwrap().read_to_end(&mut data).unwrap();
    assert_eq!(data, b"original bytes");
    close_file(&mut file).unwrap();
    assert_eq!(fs::read(&missing).unwrap(), b"original bytes");
}

#[test]
fn windows_post_creation_qualification_error_retains_the_exact_exclusive_file() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("private-journal");
    let mut work = budget();
    let mut location = Location::capture(&path, &mut work, &|| false).unwrap();
    admit_security(&mut work);
    let file = create_private_write_through(&location.parent, &mut location.name)
        .unwrap_or_else(|failure| panic!("private creation failed: {}", failure.cause));
    let expected = identity(&file).unwrap();
    // Exercise the exact post-open ownership boundary with a deterministic
    // metadata qualification failure, without manufacturing an invalid handle.
    let mut failed = retain_private_qualification(file, |_| Err(IoError::Io(ErrorKind::Other)))
        .err()
        .unwrap();
    assert_eq!(failed.cause, IoError::Io(ErrorKind::Other));
    assert!(failed.entry_may_remain);
    assert!(identity(failed.file.as_ref().unwrap()).unwrap() == expected);
    assert!(fs::File::open(&path).is_err());
    close_file(&mut failed.file).unwrap();
    assert!(failed.file.is_none());
    assert_eq!(fs::read(&path).unwrap(), b"");
}

//! Host routing preserves required publication's captured owner and outcome states.
use super::budget;
use crate::native::publication::{Error, Output};
use std::{fs, io::ErrorKind};
#[cfg(unix)]
use tablecraft_core::limits::Resource;
use tablecraft_io::{IoError, publication::PublicationError};

#[test]
fn visible_and_uncertain_errors_never_claim_absence_or_rollback() {
    for (error, state) in [
        (
            PublicationError::VisibilityUncertain {
                cause: IoError::Io(ErrorKind::Other),
            },
            "visibility is uncertain",
        ),
        (
            PublicationError::PublishedUnconfirmed {
                cause: IoError::Io(ErrorKind::Other),
            },
            "output is published but namespace durability is unconfirmed",
        ),
        (
            PublicationError::PublishedCleanupFailure {
                cause: IoError::Io(ErrorKind::Other),
                namespace_synced: true,
                barrier: None,
            },
            "output is published; temporary cleanup failed",
        ),
    ] {
        let report = Error::Required(error).to_string();
        assert!(report.contains(state), "{report}");
        assert!(!report.contains("no final output"));
        assert!(!report.contains("not published"));
        assert!(!report.contains("rolled back"));
    }
    let retained = Error::Required(PublicationError::NotPublished {
        cause: IoError::Cancelled,
        cleanup: Some(IoError::Io(ErrorKind::PermissionDenied)),
        temporary_may_remain: true,
    })
    .to_string();
    assert!(retained.contains("output was not published"));
    assert!(retained.contains("temporary may remain: true"));
}

#[cfg(unix)]
#[test]
fn required_wrapper_does_not_charge_the_requested_payload_twice() {
    let directory = tempfile::tempdir().unwrap();
    let mut used = Vec::new();
    for (name, payload) in [
        ("copy-a.accdb", b"abc".as_slice()),
        ("copy-b.accdb", b"abcd".as_slice()),
    ] {
        let path = directory.path().join(name);
        let mut work = budget();
        let before = work.remaining(Resource::Bytes);
        let output = Output::capture(&path, true, &mut work, &|| false).unwrap();
        output.publish(payload, &mut work, &|| false).unwrap();
        used.push(before - work.remaining(Resource::Bytes));
        assert_eq!(fs::read(&path).unwrap(), payload);
    }
    // Fixed-width names and identical protocol phases isolate one extra payload
    // byte. A second wrapper charge would increase this difference to two.
    assert_eq!(used[1] - used[0], 1);
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 2);
}

#[cfg(unix)]
#[test]
fn required_wrapper_uses_the_held_parent_after_textual_path_retargeting() {
    let directory = tempfile::tempdir().unwrap();
    let original = directory.path().join("parent");
    let held = directory.path().join("captured-parent");
    fs::create_dir(&original).unwrap();
    let path = original.join("copy.accdb");
    let mut work = budget();
    let output = Output::capture(&path, true, &mut work, &|| false).unwrap();
    fs::rename(&original, &held).unwrap();
    fs::create_dir(&original).unwrap();

    output.publish(b"COPY", &mut work, &|| false).unwrap();
    assert_eq!(fs::read(held.join("copy.accdb")).unwrap(), b"COPY");
    assert!(!path.exists());
    assert_eq!(fs::read_dir(&original).unwrap().count(), 0);
    assert_eq!(fs::read_dir(&held).unwrap().count(), 1);
}

#[cfg(not(unix))]
#[test]
fn unsupported_required_backend_refuses_without_staging() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("copy.accdb");
    assert!(matches!(
        Output::capture(&path, true, &mut budget(), &|| false),
        Err(Error::Required(PublicationError::UnsupportedDurability))
    ));
    assert!(!path.exists());
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 0);
}

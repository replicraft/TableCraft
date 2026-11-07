//! These tests require a local NTFS temporary directory and execute only on
//! Windows. Cross-compiling them does not establish filesystem behavior.

use super::super::{PublicationError, PublicationGuarantee, RequiredOutput};
use crate::IoError;
use std::cell::Cell;
use std::fs;
use std::io::ErrorKind;
use tablecraft_core::limits::ResourceBudget;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000, 0, 1_000_000)
}

#[test]
fn windows_publication_renames_exact_payload_without_replacing_existing_output() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("Database1.accdb");
    let captured = RequiredOutput::capture(&output, &mut budget(), &|| false).unwrap();
    let receipt = captured.publish(b"COPY", &mut budget(), &|| false).unwrap();

    assert_eq!(
        receipt.guarantee(),
        PublicationGuarantee::NtfsWriteThroughRename
    );
    assert!(!receipt.namespace_synced());
    assert_eq!(fs::read(&output).unwrap(), b"COPY");
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);

    let captured = RequiredOutput::capture(&output, &mut budget(), &|| false).unwrap();
    let failure = captured.publish(b"OTHER", &mut budget(), &|| false);
    assert!(matches!(
        failure,
        Err(PublicationError::NotPublished {
            cause: IoError::Io(ErrorKind::AlreadyExists),
            temporary_may_remain: false,
            ..
        })
    ));
    assert_eq!(fs::read(&output).unwrap(), b"COPY");
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
}

#[test]
fn windows_publication_cancellation_before_visibility_removes_only_staging() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("Database1.accdb");
    let captured = RequiredOutput::capture(&output, &mut budget(), &|| false).unwrap();
    let staging_seen = Cell::new(false);
    let result = captured.publish(b"COPY", &mut budget(), &|| {
        let exists = fs::read_dir(directory.path()).unwrap().next().is_some();
        staging_seen.set(staging_seen.get() || exists);
        exists
    });

    assert!(matches!(
        result,
        Err(PublicationError::NotPublished {
            cause: IoError::Cancelled,
            temporary_may_remain: false,
            ..
        })
    ));
    assert!(staging_seen.get());
    assert!(!output.exists());
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 0);
}

#[test]
fn windows_publication_rejects_unsafe_names_before_creating_any_entry() {
    let directory = tempfile::tempdir().unwrap();
    for name in [
        "CON.accdb",
        "data.accdb:stream",
        "data.accdb.",
        "data.accdb ",
        "result\\.",
        "result\\..",
    ] {
        let result =
            RequiredOutput::capture(&directory.path().join(name), &mut budget(), &|| false);
        assert!(matches!(
            result,
            Err(PublicationError::NotPublished {
                cause: IoError::Io(ErrorKind::InvalidInput),
                temporary_may_remain: false,
                ..
            })
        ));
    }
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 0);
}

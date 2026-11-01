use super::{CreationDestination, CreationError, budget};
use std::{
    cell::{Cell, RefCell},
    fs, io,
    path::Path,
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_io::{
    IoError,
    publication::{PublicationError, RequiredOutput},
};
use tablecraft_testkit::access::canonical_creation::{self, Case};
use tablecraft_testkit::access::canonical_native17_creation as native17;

#[test]
fn creation_receipt_retains_the_captured_parent_after_textual_path_retargeting() {
    let mut source = canonical_creation::source(Case::One);
    source.bytes = native17::source(source.bytes);
    let root = tempfile::tempdir().unwrap();
    let original_parent = root.path().join("original-parent");
    fs::create_dir(&original_parent).unwrap();
    let identity_probe = RequiredOutput::capture(
        &original_parent.join("identity-probe"),
        &mut budget(),
        &|| false,
    )
    .unwrap()
    .publish(b"same captured parent", &mut budget(), &|| false)
    .unwrap();
    let expected_parent = identity_probe.parent_identity().clone();
    let destination =
        CreationDestination::capture(&original_parent.join("new.accdb"), &mut budget(), &|| false)
            .unwrap();
    let held_parent = root.path().join("held-parent");
    fs::rename(&original_parent, &held_parent).unwrap();
    fs::create_dir(&original_parent).unwrap();
    fs::write(
        original_parent.join("new.accdb"),
        b"retain replacement entry",
    )
    .unwrap();
    let columns = source
        .columns
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let rows = source.rows.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let prepared = destination
        .prepare(source.table, &columns, &rows, &mut budget(), &|| false)
        .unwrap();
    let receipt = prepared.publish(&mut budget(), &|| false).unwrap();
    assert!(receipt.namespace_synced());
    assert!(receipt.parent_identity() == &expected_parent);
    assert_eq!((receipt.row_count(), receipt.column_count()), (1, 2));
    assert_eq!(
        fs::read(held_parent.join("new.accdb")).unwrap(),
        source.bytes
    );
    assert_eq!(
        fs::read(original_parent.join("new.accdb")).unwrap(),
        b"retain replacement entry"
    );
    assert_eq!(
        fs::read(held_parent.join("identity-probe")).unwrap(),
        b"same captured parent"
    );
    assert_eq!(fs::read_dir(&held_parent).unwrap().count(), 2);
    assert_eq!(fs::read_dir(&original_parent).unwrap().count(), 1);
}

#[derive(Clone, Copy)]
enum Refusal {
    EarlyCancellation,
    Bytes,
    Steps,
    AfterPayloadWrite,
}

fn complete_private_payload(parent: &Path, length: u64) -> io::Result<bool> {
    for entry in fs::read_dir(parent)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        for child in fs::read_dir(entry.path())? {
            let child = child?;
            if child.file_type()?.is_file() && child.metadata()?.len() == length {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

#[test]
fn prepared_creation_cancellation_and_publish_limits_leave_no_final_or_staged_file() {
    for mode in [
        Refusal::EarlyCancellation,
        Refusal::Bytes,
        Refusal::Steps,
        Refusal::AfterPayloadWrite,
    ] {
        let mut source = canonical_creation::source(Case::One);
        source.bytes = native17::source(source.bytes);
        let root = tempfile::tempdir().unwrap();
        let output = root.path().join("new.accdb");
        let destination = CreationDestination::capture(&output, &mut budget(), &|| false).unwrap();
        let columns = source
            .columns
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        let rows = source.rows.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let prepared = destination
            .prepare(source.table, &columns, &rows, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
        let mut publish_budget = match mode {
            Refusal::Bytes => ResourceBudget::new(0, 100_000, 100_000_000),
            Refusal::Steps => ResourceBudget::new(256 * 1024 * 1024, 100_000, 0),
            _ => budget(),
        };
        let observed_payload = Cell::new(false);
        let observation_error = RefCell::new(None);
        let cancelled = || {
            if matches!(mode, Refusal::EarlyCancellation) {
                return true;
            }
            if matches!(mode, Refusal::AfterPayloadWrite) {
                match complete_private_payload(root.path(), source.bytes.len() as u64) {
                    Ok(true) => {
                        observed_payload.set(true);
                        return true;
                    }
                    Ok(false) => {}
                    Err(error) => {
                        *observation_error.borrow_mut() = Some(error);
                        return true;
                    }
                }
            }
            false
        };
        let error = prepared
            .publish(&mut publish_budget, &cancelled)
            .expect_err("an unpublished creation must reject cancellation or insufficient work");
        assert!(observation_error.borrow().is_none());
        match error {
            CreationError::Publication(PublicationError::NotPublished {
                cause,
                cleanup,
                temporary_may_remain,
            }) => {
                assert_eq!(cleanup, None);
                assert!(!temporary_may_remain);
                match mode {
                    Refusal::EarlyCancellation | Refusal::AfterPayloadWrite => {
                        assert_eq!(cause, IoError::Cancelled);
                    }
                    Refusal::Bytes => {
                        assert!(
                            matches!(cause, IoError::Limit(error) if error.resource == Resource::Bytes)
                        );
                    }
                    Refusal::Steps => {
                        assert!(
                            matches!(cause, IoError::Limit(error) if error.resource == Resource::Steps)
                        );
                    }
                }
            }
            other => panic!("publication state must remain NotPublished: {other:?}"),
        }
        if matches!(mode, Refusal::AfterPayloadWrite) {
            assert!(
                observed_payload.get(),
                "the test must reach an actual private payload write"
            );
        }
        assert!(!output.exists());
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
    }
}

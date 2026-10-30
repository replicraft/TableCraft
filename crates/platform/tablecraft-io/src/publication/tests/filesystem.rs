use super::super::{PublicationError, RequiredOutput};
use crate::IoError;
use std::cell::Cell;
use std::fs;
use std::io::ErrorKind;
use tablecraft_core::limits::{Resource, ResourceBudget};

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000, 0, 1_000_000)
}

#[test]
fn required_output_publishes_exact_bytes_without_clobber_or_extra_payload_charge() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("result");
    let mut allowance = budget();
    let captured = RequiredOutput::capture(&output, &mut allowance, &|| false).unwrap();
    let before = allowance.remaining(Resource::Bytes);
    let receipt = captured
        .publish(b"COPY", &mut allowance, &|| false)
        .unwrap();
    assert!(receipt.namespace_synced());
    assert_eq!(before - allowance.remaining(Resource::Bytes), 4);
    assert_eq!(fs::read(&output).unwrap(), b"COPY");
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);

    let captured = RequiredOutput::capture(&output, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        captured.publish(b"OTHER", &mut budget(), &|| false),
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
fn required_output_anchors_actual_parent_when_text_path_is_retargeted() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("parent");
    let old = root.path().join("captured");
    fs::create_dir(&path).unwrap();
    let captured = RequiredOutput::capture(&path.join("result"), &mut budget(), &|| false).unwrap();
    fs::rename(&path, &old).unwrap();
    fs::create_dir(&path).unwrap();

    let receipt = captured.publish(b"COPY", &mut budget(), &|| false).unwrap();
    assert!(receipt.namespace_synced());
    assert_eq!(fs::read(old.join("result")).unwrap(), b"COPY");
    assert!(!path.join("result").exists());
    assert_eq!(fs::read_dir(&old).unwrap().count(), 1);
}

#[test]
fn required_output_previsibility_cancel_and_exhaustion_leave_no_output() {
    let root = tempfile::tempdir().unwrap();
    for limit in [0, 4, 8, 15] {
        let output = root.path().join("result");
        let captured = RequiredOutput::capture(&output, &mut budget(), &|| false).unwrap();
        let result = captured.publish(b"COPY", &mut ResourceBudget::new(4, 0, limit), &|| false);
        assert!(matches!(result, Err(PublicationError::NotPublished { .. })));
        assert!(!output.exists());
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
    }

    let output = root.path().join("result");
    let captured = RequiredOutput::capture(&output, &mut budget(), &|| false).unwrap();
    let polls = Cell::new(0);
    let result = captured.publish(b"COPY", &mut budget(), &|| {
        polls.set(polls.get() + 1);
        polls.get() == 4
    });
    assert!(matches!(
        result,
        Err(PublicationError::NotPublished {
            cause: IoError::Cancelled,
            ..
        })
    ));
    assert!(!output.exists());
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
}

#[test]
fn required_output_invalid_components_and_capture_cancellation_have_no_effects() {
    let root = tempfile::tempdir().unwrap();
    for path in [
        root.path().join(".."),
        root.path().join("bad\0name"),
        root.path().join("result/."),
        root.path().join("result/"),
    ] {
        assert!(matches!(
            RequiredOutput::capture(&path, &mut budget(), &|| false),
            Err(PublicationError::NotPublished {
                cause: IoError::Io(ErrorKind::InvalidInput),
                ..
            })
        ));
    }
    assert!(matches!(
        RequiredOutput::capture(&root.path().join("result"), &mut budget(), &|| true),
        Err(PublicationError::NotPublished {
            cause: IoError::Cancelled,
            ..
        })
    ));
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
}

#[test]
fn required_output_exact_and_one_below_pools_preserve_the_visibility_boundary() {
    let root = tempfile::tempdir().unwrap();
    let output = root.path().join("result");
    let captured = RequiredOutput::capture(&output, &mut budget(), &|| false).unwrap();
    let mut measured = budget();
    let start = measured.remaining(Resource::Steps);
    captured.publish(b"COPY", &mut measured, &|| false).unwrap();
    let used = start - measured.remaining(Resource::Steps);
    fs::remove_file(&output).unwrap();

    for steps in [used - 1, used] {
        let captured = RequiredOutput::capture(&output, &mut budget(), &|| false).unwrap();
        let result = captured.publish(b"COPY", &mut ResourceBudget::new(4, 0, steps), &|| false);
        assert_eq!(result.is_ok(), steps == used);
        if steps == used {
            assert_eq!(fs::read(&output).unwrap(), b"COPY");
            fs::remove_file(&output).unwrap();
        }
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
    }

    let captured = RequiredOutput::capture(&output, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        captured.publish(b"COPY", &mut ResourceBudget::new(3, 0, used), &|| false),
        Err(PublicationError::NotPublished { .. })
    ));
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
}

#[test]
fn required_output_long_parent_buffers_are_paid_before_capture_effects() {
    let root = tempfile::tempdir().unwrap();
    let mut parent = root.path().to_path_buf();
    for _ in 0..3 {
        parent.push("p".repeat(100));
    }
    fs::create_dir_all(&parent).unwrap();
    let output = parent.join("x");
    let needed = parent.as_os_str().len() as u64 + 1 + 2;
    assert!(parent.as_os_str().len() > 256);

    let result = RequiredOutput::capture(
        &output,
        &mut ResourceBudget::new(needed - 1, 0, 1_000_000),
        &|| false,
    );
    assert!(matches!(result, Err(PublicationError::NotPublished {
        cause: IoError::Limit(error), temporary_may_remain: false, ..
    }) if error.resource == Resource::Bytes));
    assert_eq!(fs::read_dir(&parent).unwrap().count(), 0);

    let mut exact = ResourceBudget::new(needed, 0, 1_000_000);
    assert!(RequiredOutput::capture(&output, &mut exact, &|| false).is_ok());
    assert_eq!(exact.remaining(Resource::Bytes), 0);
    assert_eq!(fs::read_dir(&parent).unwrap().count(), 0);
}

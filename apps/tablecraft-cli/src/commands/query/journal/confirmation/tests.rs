use super::{Error, Reports, VerifiedImage};
use std::io::{self, Write};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::RecoveryOutcome;

struct FailedOutput {
    fail_write: bool,
    bytes: Vec<u8>,
}

impl Write for FailedOutput {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.fail_write {
            return Err(io::ErrorKind::BrokenPipe.into());
        }

        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Err(io::ErrorKind::BrokenPipe.into())
    }
}

#[test]
fn verified_reports_identify_current_images_without_claiming_historical_events() {
    for (outcome, image, expected) in [
        (
            RecoveryOutcome::Committed,
            VerifiedImage::Replacement,
            b"Verified image\tReplacement\nJournal namespace\tRetired and synchronized\n"
                .as_slice(),
        ),
        (
            RecoveryOutcome::RolledBack,
            VerifiedImage::Original,
            b"Verified image\tOriginal\nJournal namespace\tRetired and synchronized\n".as_slice(),
        ),
    ] {
        assert_eq!(VerifiedImage::from_confirmation(outcome), image);
        let reports = Reports::prepare(&mut ResourceBudget::new(4096, 0, 4096)).unwrap();
        let mut bytes = Vec::new();
        reports.publish(image, &mut bytes).unwrap();
        assert_eq!(bytes, expected);
    }
}

#[test]
fn confirmation_output_failure_preserves_verified_image_without_recovery_labels() {
    for image in [VerifiedImage::Original, VerifiedImage::Replacement] {
        for fail_write in [true, false] {
            let reports = Reports::prepare(&mut ResourceBudget::new(4096, 0, 4096)).unwrap();
            let mut writer = FailedOutput {
                fail_write,
                bytes: Vec::new(),
            };
            let failure = reports.publish(image, &mut writer).unwrap_err();
            assert!(
                matches!(&failure, Error::VerifiedOutput { image: actual, error }
                if *actual == image && error.kind() == io::ErrorKind::BrokenPipe)
            );
            assert!(!failure.invalid_input());
            let diagnostic = failure.to_string();
            assert!(diagnostic.contains(&format!("verified target image {image}")));
            assert!(!diagnostic.contains("Committed"));
            assert!(!diagnostic.contains("RolledBack"));
            assert_eq!(writer.bytes.is_empty(), fail_write);
        }
    }
}

#[test]
fn namespace_failure_reports_verified_image_without_reinterpreting_io_outcomes() {
    use tablecraft_io::journal::filesystem::{FinalizationError, FinalizationPhase, PairError};

    for (image, outcome) in [
        (VerifiedImage::Original, RecoveryOutcome::RolledBack),
        (VerifiedImage::Replacement, RecoveryOutcome::Committed),
    ] {
        let failure = Error::VerifiedFinalization {
            image,
            error: FinalizationError {
                phase: FinalizationPhase::GarbageCleanup,
                outcome: Some(outcome),
                cause: PairError::InvalidState,
            },
        };
        let diagnostic = failure.to_string();
        assert!(diagnostic.contains(&format!("verified target image {image}")));
        assert!(diagnostic.contains("GarbageCleanup"));
        assert!(!diagnostic.contains("Committed"));
        assert!(!diagnostic.contains("RolledBack"));
        assert!(!failure.invalid_input());
    }
}

#[test]
fn finalization_arguments_accept_no_sql_or_copy_mode() {
    use clap::Parser;

    let base = [
        "tablecraft",
        "query",
        "finalize-update",
        "--database",
        "offline.mdb",
        "--journal",
        "retired-directory",
    ];
    assert!(crate::Cli::try_parse_from(base).is_ok());
    for extra in ["UPDATE Items SET Score=12", "--output", "--durable-output"] {
        assert!(crate::Cli::try_parse_from(base.into_iter().chain([extra])).is_err());
    }
}

#[test]
fn retired_open_error_slot_is_prepaid_and_retains_the_complete_failure_owner() {
    use super::reserve_open_failure;
    use tablecraft_ace::{InspectError, NativeInt32EditIntent};
    use tablecraft_core::limits::Resource;
    use tablecraft_io::{
        IoError, SharedFile,
        journal::filesystem::{OfflineJournalPair, OfflineTarget, PairSetupFailure},
    };

    let bytes = size_of::<PairSetupFailure>() as u64;
    let mut exact = ResourceBudget::new(bytes, 0, 1);
    let mut retained = reserve_open_failure(&mut exact).unwrap();
    assert!(retained.is_empty());
    assert!(retained.capacity() >= 1);
    assert_eq!(exact.remaining(Resource::Bytes), 0);
    assert_eq!(exact.remaining(Resource::Steps), 0);

    for (bytes, steps, resource) in [(bytes - 1, 1, Resource::Bytes), (bytes, 0, Resource::Steps)] {
        let failure = reserve_open_failure(&mut ResourceBudget::new(bytes, 0, steps))
            .expect_err("one-below slot allowance must fail before opening a target");
        assert!(matches!(
            failure,
            Error::ConfirmationResources(InspectError::Limit(error))
                if error.resource == resource
        ));
    }

    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("target");
    std::fs::write(&path, b"original resource").unwrap();
    let mut work = ResourceBudget::new(100_000, 0, 100_000);
    let target = OfflineTarget::open(&path, &mut work, &|| false).unwrap();
    let failure = match OfflineJournalPair::open_retired(
        target,
        &root.path().join("missing-retired"),
        NativeInt32EditIntent::MAX_ENCODED_BYTES as u64,
        &mut work,
        &|| false,
    ) {
        Ok(_) => panic!("missing retired directory unexpectedly opened"),
        Err(error) => error,
    };
    let cause = failure.cause().to_string();
    let allocation = retained.as_ptr();
    retained.push(failure);
    assert_eq!(retained.as_ptr(), allocation);
    let wrapped = Error::RetiredOpen(retained);
    assert!(wrapped.to_string().contains(&cause));
    assert!(matches!(SharedFile::open(&path), Err(IoError::LockBusy)));
    drop(wrapped);
    assert!(SharedFile::open(&path).is_ok());
    assert_eq!(std::fs::read(path).unwrap(), b"original resource");

    eprintln!(
        "journal error bytes: {}; execution error bytes: {}",
        size_of::<Error>(),
        size_of::<crate::commands::ExecutionError>()
    );
}

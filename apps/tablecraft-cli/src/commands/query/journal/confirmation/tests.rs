use super::super::report::NamespaceFacts;
use super::{Error, Reports, VerifiedImage};
use std::io::{self, Write};
use tablecraft_access::editing::offline::VerifiedImage as NativeVerifiedImage;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::filesystem_contracts::{CleanupDisposition, RetirementGuarantee};

fn namespace() -> NamespaceFacts {
    NamespaceFacts {
        retirement: Some(RetirementGuarantee::CapturedParentBarrier),
        cleanup: Some(CleanupDisposition::Removed),
    }
}

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
            NativeVerifiedImage::Replacement,
            VerifiedImage::Replacement,
            b"Verified image\tReplacement\nJournal namespace\tRetired\nRetirement guarantee\tCaptured-parent barrier\nJournal cleanup\tRemoved\n"
                .as_slice(),
        ),
        (
            NativeVerifiedImage::Original,
            VerifiedImage::Original,
            b"Verified image\tOriginal\nJournal namespace\tRetired\nRetirement guarantee\tCaptured-parent barrier\nJournal cleanup\tRemoved\n".as_slice(),
        ),
    ] {
        assert_eq!(VerifiedImage::from_confirmation(outcome), image);
        let reports = Reports::prepare(&mut ResourceBudget::new(4096, 0, 4096)).unwrap();
        let mut bytes = Vec::new();
        reports.publish(image, namespace(), &mut bytes).unwrap();
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
            let failure = reports
                .publish(image, namespace(), &mut writer)
                .unwrap_err();
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

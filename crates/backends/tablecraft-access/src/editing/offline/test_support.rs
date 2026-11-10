//! Adapter-private receipt and held-lease assertions shared by native save tests.
#[cfg(feature = "ace16-create")]
pub(super) mod native_creation;
#[cfg(feature = "ace16-create")]
pub(super) mod saved_field;

use super::Receipt;
#[cfg(any(unix, feature = "ace16-create"))]
use std::fs;
use std::path::Path;
use tablecraft_io::journal::filesystem_contracts::{CleanupDisposition, RetirementGuarantee};
#[cfg(windows)]
use tablecraft_io::{IoError, SharedFile};

pub(super) fn assert_staged_read_state(path: &Path, expected: &[u8]) {
    #[cfg(unix)]
    assert_eq!(fs::read(path).unwrap(), expected);
    #[cfg(windows)]
    {
        let _ = expected;
        assert!(matches!(SharedFile::open(path), Err(IoError::LockBusy)));
    }
}

pub(super) fn assert_completion(receipt: &Receipt) {
    #[cfg(unix)]
    {
        assert_eq!(
            receipt.retirement(),
            Some(RetirementGuarantee::CapturedParentBarrier)
        );
        assert_eq!(receipt.cleanup(), Some(CleanupDisposition::Removed));
    }
    #[cfg(windows)]
    {
        assert_eq!(
            receipt.retirement(),
            Some(RetirementGuarantee::NtfsWriteThroughRename)
        );
        assert_eq!(
            receipt.cleanup(),
            Some(CleanupDisposition::RetainedArtifact)
        );
    }
}

#[cfg(feature = "ace16-create")]
pub(super) fn assert_terminal_entries(directory: &Path, retired_count: usize) {
    #[cfg(unix)]
    let expected = {
        let _ = retired_count;
        1
    };
    #[cfg(windows)]
    let expected = 1 + retired_count;
    assert_eq!(fs::read_dir(directory).unwrap().count(), expected);
}

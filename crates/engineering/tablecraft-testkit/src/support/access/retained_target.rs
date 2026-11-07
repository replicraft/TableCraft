//! Path observations while an actual offline write target remains retained.
use std::path::Path;

#[cfg(windows)]
const WINDOWS_SHARING_VIOLATION: i32 = 32;

/// Observe the platform's retained-target behavior without opening a back door
/// around its write lease. Windows denies path-based reads; Unix permits an
/// ordinary read but cooperative lock consumers must still respect the lease.
/// Callers must verify the complete bytes again after acknowledged release.
pub fn assert_retained_target_observation(path: &Path, expected: &[u8]) {
    #[cfg(windows)]
    {
        let _ = expected;
        let error = std::fs::read(path).expect_err("retained target must deny read sharing");
        assert_eq!(error.raw_os_error(), Some(WINDOWS_SHARING_VIOLATION));
    }

    #[cfg(not(windows))]
    assert_eq!(std::fs::read(path).unwrap(), expected);
}

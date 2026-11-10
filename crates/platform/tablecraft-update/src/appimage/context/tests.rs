use super::{RuntimeContext, from_environment};
use crate::{installation::ManualReason, platform::Platform};
use std::ffi::OsStr;

#[test]
fn environment_only_selects_candidates_and_rejects_incomplete_or_development_context() {
    assert!(matches!(
        from_environment(Platform::LinuxX64, None, None, false),
        RuntimeContext::Absent
    ));
    for (image, appdir) in [
        (Some("/images/app"), None),
        (Some("relative"), Some("/mount")),
    ] {
        assert!(matches!(
            from_environment(
                Platform::LinuxX64,
                image.map(OsStr::new),
                appdir.map(OsStr::new),
                false
            ),
            RuntimeContext::Manual(ManualReason::UnrecognizedLayout)
        ));
    }
    assert!(matches!(
        from_environment(
            Platform::LinuxArm64,
            Some(OsStr::new("/image")),
            Some(OsStr::new("/mount")),
            true
        ),
        RuntimeContext::Manual(ManualReason::DevelopmentBuild)
    ));
    assert!(matches!(
        from_environment(
            Platform::LinuxArm64,
            Some(OsStr::new("/image")),
            Some(OsStr::new("/mount")),
            false
        ),
        RuntimeContext::Candidate(_)
    ));
}

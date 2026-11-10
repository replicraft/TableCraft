use crate::{installation::ManualReason, platform::Platform};
use std::{ffi::OsStr, path::PathBuf};

pub(crate) enum RuntimeContext {
    Absent,
    Manual(ManualReason),
    Candidate(Context),
}

pub(crate) struct Context {
    pub(super) image: PathBuf,
    pub(super) appdir: PathBuf,
}

pub(crate) fn capture(platform: Platform) -> RuntimeContext {
    let image = std::env::var_os("APPIMAGE");
    let appdir = std::env::var_os("APPDIR");
    from_environment(
        platform,
        image.as_deref(),
        appdir.as_deref(),
        cfg!(debug_assertions),
    )
}

fn from_environment(
    platform: Platform,
    image: Option<&OsStr>,
    appdir: Option<&OsStr>,
    debug_build: bool,
) -> RuntimeContext {
    if !matches!(platform, Platform::LinuxX64 | Platform::LinuxArm64) {
        return RuntimeContext::Absent;
    }
    let image = image.filter(|value| !value.is_empty());
    let appdir = appdir.filter(|value| !value.is_empty());
    if image.is_none() && appdir.is_none() {
        return RuntimeContext::Absent;
    }
    if debug_build {
        return RuntimeContext::Manual(ManualReason::DevelopmentBuild);
    }
    let (Some(image), Some(appdir)) = (image, appdir) else {
        return RuntimeContext::Manual(ManualReason::UnrecognizedLayout);
    };
    let image = PathBuf::from(image);
    let appdir = PathBuf::from(appdir);
    if !image.is_absolute() || !appdir.is_absolute() {
        return RuntimeContext::Manual(ManualReason::UnrecognizedLayout);
    }
    RuntimeContext::Candidate(Context { image, appdir })
}

#[cfg(test)]
mod tests;

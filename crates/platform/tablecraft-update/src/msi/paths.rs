use crate::UpdateError;
use std::path::{Path, PathBuf};

#[cfg(windows)]
use std::{
    os::windows::ffi::OsStrExt,
    path::{Component, Prefix},
};

#[cfg(windows)]
const MAX_LEGACY_PATH_UTF16: usize = 260;

pub(super) fn canonicalize(path: &Path) -> Result<PathBuf, UpdateError> {
    if path.to_str().is_none() {
        return Err(UpdateError::UnqualifiedInstallation);
    }
    #[cfg(windows)]
    {
        let local_drive = matches!(path.components().next(), Some(Component::Prefix(prefix)) if matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_)));
        if !path.is_absolute() || !local_drive {
            return Err(UpdateError::UnqualifiedInstallation);
        }
    }
    #[cfg(windows)]
    let canonical = dunce::canonicalize(path).map_err(UpdateError::Io)?;
    #[cfg(not(windows))]
    let canonical = std::fs::canonicalize(path).map_err(UpdateError::Io)?;
    ensure_compatible(&canonical)?;
    Ok(canonical)
}

pub(super) fn ensure_compatible(path: &Path) -> Result<(), UpdateError> {
    if path.to_str().is_none() {
        return Err(UpdateError::UnqualifiedInstallation);
    }
    #[cfg(windows)]
    {
        let local_drive = matches!(path.components().next(), Some(Component::Prefix(prefix)) if matches!(prefix.kind(), Prefix::Disk(_)));
        if !path.is_absolute()
            || !local_drive
            || path.as_os_str().encode_wide().count() >= MAX_LEGACY_PATH_UTF16
        {
            return Err(UpdateError::UnqualifiedInstallation);
        }
    }
    Ok(())
}

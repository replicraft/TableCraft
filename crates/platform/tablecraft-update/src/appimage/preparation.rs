use super::{InstalledAppImage, payload};
use crate::{
    UpdateError,
    apply::{AppliedUpdate, ApplyError, transaction},
    artifact::ArtifactKind,
    download::VerifiedDownload,
};
use semver::Version;
use std::{cmp::Ordering, fs};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[cfg(unix)]
const ORDINARY_ACCESS_BITS: u32 = 0o777;
#[cfg(unix)]
const OWNER_EXECUTE: u32 = 0o100;

#[derive(Debug)]
pub(crate) struct PreparedAppImage {
    installed: InstalledAppImage,
    download: VerifiedDownload,
}

impl PreparedAppImage {
    pub(crate) fn prepare(
        download: VerifiedDownload,
        installed: InstalledAppImage,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, UpdateError> {
        if download.kind() != ArtifactKind::AppImage
            || download.platform() != installed.platform()
            || download.version().cmp_precedence(installed.version()) != Ordering::Greater
        {
            return Err(UpdateError::ReleaseMetadata);
        }
        installed.revalidate(cancelled)?;
        let mut file = download.open_verified(cancelled)?;
        payload::check(&mut file, installed.platform())?;
        executable_permissions(&file, &installed)?;
        file.sync_all().map_err(UpdateError::Io)?;
        Ok(Self {
            installed,
            download,
        })
    }

    pub(crate) fn version(&self) -> &Version {
        self.download.version()
    }

    pub(crate) fn apply(self, cancelled: &dyn Fn() -> bool) -> Result<AppliedUpdate, ApplyError> {
        self.installed
            .revalidate(cancelled)
            .map_err(ApplyError::Installation)?;
        let mut file = self
            .download
            .open_verified(cancelled)
            .map_err(ApplyError::Installation)?;
        payload::check(&mut file, self.installed.platform()).map_err(ApplyError::Installation)?;
        executable_permissions(&file, &self.installed).map_err(ApplyError::Installation)?;
        file.sync_all().map_err(|source| ApplyError::Filesystem {
            source,
            recovery: None,
        })?;
        if cancelled() {
            return Err(ApplyError::Installation(UpdateError::Cancelled));
        }
        // The file remains owned until rename/rollback completes. On success its
        // old staging pathname is absent, so Drop cannot remove the installed image.
        transaction::apply_image(
            self.installed.image(),
            self.download.staged_path(),
            self.installed.staging_parent(),
        )
    }
}

#[cfg(unix)]
fn executable_permissions(
    file: &fs::File,
    installed: &InstalledAppImage,
) -> Result<(), UpdateError> {
    // Preserve ordinary access bits, guarantee owner execution and strip special
    // privilege/sticky bits. Updating never grants setuid/setgid behavior.
    let mode = fs::metadata(installed.image())
        .map_err(UpdateError::Io)?
        .permissions()
        .mode();
    file.set_permissions(fs::Permissions::from_mode(
        (mode & ORDINARY_ACCESS_BITS) | OWNER_EXECUTE,
    ))
    .map_err(UpdateError::Io)
}

#[cfg(not(unix))]
fn executable_permissions(_: &fs::File, _: &InstalledAppImage) -> Result<(), UpdateError> {
    Err(UpdateError::UnqualifiedInstallation)
}

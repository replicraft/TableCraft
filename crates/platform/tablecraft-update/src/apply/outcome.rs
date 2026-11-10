use crate::UpdateError;
use std::{
    io,
    path::{Path, PathBuf},
    process::Child,
};
use thiserror::Error;

/// Paths retained deliberately when replacement cannot safely retire originals.
///
/// Dropping this value does not delete the backup. Display these paths only in
/// explicit recovery diagnostics, never automatic operation logs or spans.
#[derive(Debug)]
pub struct RetainedRecovery {
    pub(super) backup: PathBuf,
    pub(super) installation: PathBuf,
}

impl RetainedRecovery {
    /// Returns the private directory containing any originals not restored.
    pub fn backup_directory(&self) -> &Path {
        &self.backup
    }

    /// Returns the original installation or application-bundle parent.
    pub fn installation_directory(&self) -> &Path {
        &self.installation
    }
}

/// A completed replacement whose relaunch helper is waiting for this process.
///
/// The host must exit through its normal document-safe shutdown after receiving
/// this result. Dropping it neither kills the helper nor deletes the backup.
#[derive(Debug)]
pub struct AppliedUpdate {
    pub(super) recovery: RetainedRecovery,
    pub(super) helper: Option<Child>,
}

impl AppliedUpdate {
    /// Returns the helper process identifier after successful native spawn.
    pub fn helper_process_id(&self) -> Option<u32> {
        self.helper.as_ref().map(Child::id)
    }

    /// Returns the original-file backup retained until relaunch succeeds.
    pub fn backup_directory(&self) -> &Path {
        self.recovery.backup_directory()
    }
}

/// A failed replacement, with explicit recovery resources when rollback could not finish.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ApplyError {
    /// The running package changed or could no longer be inspected.
    #[error("The installed application could not be revalidated")]
    Installation(#[source] UpdateError),
    /// Platform layout does not match the prepared update.
    #[error("The update does not match the installed application layout")]
    Layout,
    /// Filesystem replacement or helper startup failed.
    #[error("The application update could not be installed")]
    Filesystem {
        /// The underlying operation failure, without release-controlled arguments.
        #[source]
        source: io::Error,
        /// Originals retained because rollback or backup cleanup did not finish.
        recovery: Option<RetainedRecovery>,
    },
}

impl ApplyError {
    /// Returns recovery paths when some originals or cleanup resources remain.
    pub fn retained_recovery(&self) -> Option<&RetainedRecovery> {
        match self {
            Self::Filesystem { recovery, .. } => recovery.as_ref(),
            Self::Installation(_) | Self::Layout => None,
        }
    }
}

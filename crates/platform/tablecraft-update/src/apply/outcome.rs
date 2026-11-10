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

/// The native installation phase reached before the host begins normal shutdown.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApplyStage {
    /// Package files have been replaced and the helper is waiting to relaunch.
    Replaced,
    /// A native installer helper is waiting to install after the host exits.
    ///
    /// This does not establish successful installation, elevation or upgrade.
    InstallerScheduled,
}

/// A committed update handoff whose helper is waiting for normal host shutdown.
///
/// Inspect `stage` to distinguish completed file replacement from a scheduled
/// native installer. Dropping this value never kills the helper or deletes its
/// retained originals, authenticated payload or diagnostic resources.
#[derive(Debug)]
pub struct AppliedUpdate {
    pub(super) recovery: RetainedRecovery,
    pub(super) helper: Option<Child>,
    pub(super) stage: ApplyStage,
}

impl AppliedUpdate {
    pub(crate) fn handoff(
        staging: PathBuf,
        installation: PathBuf,
        helper: Option<Child>,
        stage: ApplyStage,
    ) -> Self {
        Self {
            recovery: RetainedRecovery {
                backup: staging,
                installation,
            },
            helper,
            stage,
        }
    }

    /// Returns whether files were replaced or a native installation was scheduled.
    pub const fn stage(&self) -> ApplyStage {
        self.stage
    }

    /// Returns the helper process identifier after successful native spawn.
    pub fn helper_process_id(&self) -> Option<u32> {
        self.helper.as_ref().map(Child::id)
    }

    /// Returns retained originals or authenticated native-installer resources.
    pub fn retained_directory(&self) -> &Path {
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

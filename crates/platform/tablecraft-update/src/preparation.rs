use crate::{
    UpdateError,
    apply::{AppliedUpdate, ApplyError, transaction},
    archive::{self, ExtractedUpdate},
    download::VerifiedDownload,
    installation::Installed,
};
use semver::Version;
use std::cmp::Ordering;

/// A verified update extracted beside its captured, still-unmodified installation.
///
/// This capability is produced only from authenticated downloads and local
/// installation proofs. Preparation never replaces installed application files.
#[derive(Debug)]
pub struct PreparedUpdate {
    pub(crate) installed: Installed,
    pub(crate) extracted: ExtractedUpdate,
    version: Version,
}

impl PreparedUpdate {
    /// Replace the captured installation and schedule relaunch after normal exit.
    ///
    /// The host must resolve unsaved documents and stop database work before
    /// calling this consuming operation. This method never closes the host.
    /// Cancellation is checked before filesystem mutation; an active replacement
    /// runs to a terminal result with rollback or retained recovery resources.
    ///
    /// # Errors
    /// Refuses changed staging/source bytes, cancellation, unsupported layouts,
    /// filesystem failures and helper startup failures. Failed rollback retains
    /// original files through the returned recovery information.
    pub fn apply(self, cancelled: &dyn Fn() -> bool) -> Result<AppliedUpdate, ApplyError> {
        self.extracted
            .revalidate(self.installed.platform(), &self.version, cancelled)
            .map_err(ApplyError::Installation)?;
        if cancelled() {
            return Err(ApplyError::Installation(UpdateError::Cancelled));
        }
        transaction::apply(self.installed, self.extracted)
    }

    /// The authenticated version to be installed.
    pub fn version(&self) -> &Version {
        &self.version
    }

    /// The immutable local installation selected before download and preparation.
    pub fn installation(&self) -> &Installed {
        &self.installed
    }
}

impl VerifiedDownload {
    /// Authenticate again and extract into owned staging beside the installation.
    ///
    /// Preparation consumes the download and local installation proof. All
    /// staging is removed on cancellation or failure. No installed file changes.
    /// This blocking work belongs on a host worker.
    ///
    /// # Errors
    /// Refuses changed installation/download bytes, platform/version mismatch,
    /// cancellation, invalid archive contents and local filesystem failures.
    pub fn prepare(
        self,
        installed: Installed,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<PreparedUpdate, UpdateError> {
        if self.platform() != installed.platform()
            || self.version().cmp_precedence(installed.version()) != Ordering::Greater
            || (installed.version().pre.is_empty() && !self.version().pre.is_empty())
        {
            return Err(UpdateError::ReleaseMetadata);
        }
        installed.revalidate()?;
        let reader = self.open_verified(cancelled)?;
        let extracted = archive::extract(
            reader,
            self.platform(),
            self.version(),
            installed.directory(),
            cancelled,
        )?;
        installed.revalidate()?;
        Ok(PreparedUpdate {
            installed,
            extracted,
            version: self.into_version(),
        })
    }
}

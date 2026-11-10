use crate::{
    UpdateError,
    apply::{AppliedUpdate, ApplyError, transaction},
    archive::{self, ExtractedUpdate},
    download::VerifiedDownload,
    installation::Installed,
    msi::PreparedMsi,
    target::UpdateTarget,
};
use semver::Version;
use std::cmp::Ordering;

/// An authenticated payload prepared for its still-qualified local installation.
///
/// Preparation never changes installed application files. Each variant retains
/// its owning subsystem's target proof, payload and revalidation requirements.
#[derive(Debug)]
pub struct PreparedUpdate {
    prepared: Prepared,
}

#[derive(Debug)]
enum Prepared {
    Package(Box<PreparedPackage>),
    WindowsMsi(Box<PreparedMsi>),
}

#[derive(Debug)]
struct PreparedPackage {
    installed: Installed,
    extracted: ExtractedUpdate,
    version: Version,
}

impl PreparedUpdate {
    /// Enter the captured installation route and schedule normal-exit handoff.
    ///
    /// The host must resolve unsaved documents and stop database work before
    /// calling this consuming operation. This method never closes the host.
    /// Cancellation applies before a route commits; inspect `AppliedUpdate::stage`
    /// to distinguish file replacement from a deferred native installer.
    ///
    /// # Errors
    /// Refuses changed staging/source bytes, cancellation, unsupported layouts,
    /// filesystem failures and helper startup failures. Failed rollback retains
    /// original files or deferred-installer diagnostics in the returned resources.
    pub fn apply(self, cancelled: &dyn Fn() -> bool) -> Result<AppliedUpdate, ApplyError> {
        let _span = tracing::debug_span!("update.apply").entered();
        match self.prepared {
            Prepared::Package(prepared) => {
                let PreparedPackage {
                    installed,
                    extracted,
                    version,
                } = *prepared;
                extracted
                    .revalidate(installed.platform(), &version, cancelled)
                    .map_err(ApplyError::Installation)?;
                if cancelled() {
                    return Err(ApplyError::Installation(UpdateError::Cancelled));
                }
                transaction::apply(installed, extracted)
            }
            Prepared::WindowsMsi(prepared) => prepared.apply(cancelled),
        }
    }

    /// The authenticated version selected for this installation route.
    pub fn version(&self) -> &Version {
        match &self.prepared {
            Prepared::Package(prepared) => &prepared.version,
            Prepared::WindowsMsi(prepared) => prepared.version(),
        }
    }
}

impl VerifiedDownload {
    /// Reauthenticate and prepare a payload for its exact local target kind.
    ///
    /// Preparation consumes both capabilities. All unpublished staging is removed
    /// on cancellation or failure; no installed file changes. This blocking work
    /// belongs on a host worker rather than an event or async-executor thread.
    ///
    /// # Errors
    /// Refuses changed source/download bytes, kind/platform/version mismatch,
    /// cancellation, invalid payload contents and local filesystem failures.
    pub fn prepare(
        self,
        target: UpdateTarget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<PreparedUpdate, UpdateError> {
        let _span = tracing::debug_span!("update.prepare").entered();
        if self.kind() != target.kind()
            || self.platform() != target.platform()
            || self.version().cmp_precedence(target.version()) != Ordering::Greater
            || (target.version().pre.is_empty() && !self.version().pre.is_empty())
        {
            return Err(UpdateError::ReleaseMetadata);
        }
        let prepared = match target {
            UpdateTarget::Package(installed) => {
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
                Prepared::Package(Box::new(PreparedPackage {
                    installed,
                    extracted,
                    version: self.into_version(),
                }))
            }
            UpdateTarget::WindowsMsi(installed) => {
                Prepared::WindowsMsi(Box::new(PreparedMsi::prepare(self, installed, cancelled)?))
            }
        };
        Ok(PreparedUpdate { prepared })
    }
}

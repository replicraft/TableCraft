use crate::{
    UpdateError,
    appimage::{self, InstalledAppImage, context::Context},
    artifact::ArtifactKind,
    download::{self, DownloadProgress, VerifiedDownload},
    http::{Http, Transport},
    platform::Platform,
    release::{self, AvailableUpdate},
    trust, verification,
};
use minisign_verify::PublicKey;
use semver::Version;
use std::{cmp::Ordering, path::Path};

const LATEST_RELEASE: &str = "https://api.github.com/repos/replicraft/TableCraft/releases/latest";

/// A blocking updater with a fixed release origin and compiled publisher trust.
#[derive(Debug)]
pub struct Updater {
    version: Version,
    platform: Platform,
    key: PublicKey,
    http: Http,
}

impl Updater {
    pub(crate) fn authenticate_appimage(
        &self,
        context: Context,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<InstalledAppImage, UpdateError> {
        appimage::authenticate(
            &self.http,
            &self.key,
            context,
            &self.version,
            self.platform,
            cancelled,
        )
    }

    pub(crate) fn version(&self) -> &Version {
        &self.version
    }

    pub(crate) fn platform(&self) -> Platform {
        self.platform
    }

    /// Construct an updater for the running application and native target.
    ///
    /// # Errors
    /// Refuses missing/malformed compiled trust or HTTP client initialization.
    /// Construction and all subsequent operations belong on a worker thread.
    pub fn new(version: Version, platform: Platform) -> Result<Self, UpdateError> {
        let key = trust::release_key()?;
        let http = Http::new(&version.to_string())?;
        Ok(Self {
            version,
            platform,
            key,
            http,
        })
    }

    /// Check the fixed release endpoint for a newer matching signed asset set.
    ///
    /// # Errors
    /// Refuses cancellation, network failures and invalid or oversized metadata.
    pub fn check(
        &self,
        kind: ArtifactKind,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Option<AvailableUpdate>, UpdateError> {
        let _span = tracing::debug_span!("update.check").entered();
        check(&self.http, &self.version, self.platform, kind, cancelled)
    }

    /// Download and authenticate an update into an owned directory beside the installation.
    ///
    /// `parent` is selected by installation detection, never release metadata.
    /// Returning `false` from progress cancels and removes unpublished staging.
    ///
    /// # Errors
    /// Refuses download/size/signature failures, cancellation and staging failures.
    pub fn download(
        &self,
        available: &AvailableUpdate,
        parent: &Path,
        progress: &mut dyn FnMut(DownloadProgress) -> bool,
    ) -> Result<VerifiedDownload, UpdateError> {
        let _span = tracing::debug_span!("update.download").entered();
        if available.platform() != self.platform
            || available.version().cmp_precedence(&self.version) != Ordering::Greater
            || (self.version.pre.is_empty() && !available.version().pre.is_empty())
        {
            return Err(UpdateError::ReleaseMetadata);
        }
        download::download(&self.http, &self.key, available, parent, progress)
    }
}

fn check(
    transport: &dyn Transport,
    current: &Version,
    platform: Platform,
    kind: ArtifactKind,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<AvailableUpdate>, UpdateError> {
    if cancelled() {
        return Err(UpdateError::Cancelled);
    }
    let mut response = match transport.get(LATEST_RELEASE) {
        Ok(response) => response,
        Err(UpdateError::NotFound) => return Ok(None),
        Err(error) => return Err(error),
    };
    if response
        .length
        .is_some_and(|size| size > release::MAX_METADATA_BYTES as u64)
    {
        return Err(UpdateError::ReleaseTooLarge);
    }
    let bytes = verification::bounded_bytes(
        &mut response.reader,
        release::MAX_METADATA_BYTES,
        &mut || cancelled(),
    )
    .map_err(|error| match error {
        UpdateError::AssetTooLarge => UpdateError::ReleaseTooLarge,
        other => other,
    })?;
    release::select(&bytes, current, platform, kind)
}

#[cfg(test)]
mod tests;

//! Bounded GitHub release selection with exact product and platform identities.

use crate::{UpdateError, platform::Platform};
use semver::Version;
use serde::Deserialize;
use std::cmp::Ordering;

/// The maximum accepted GitHub release metadata body, independent of asset sizes.
pub const MAX_METADATA_BYTES: usize = 1024 * 1024;
/// The largest archive accepted by the desktop updater.
pub const MAX_ARCHIVE_BYTES: u64 = 512 * 1024 * 1024;
/// The maximum Minisign sidecar size accepted before decoding.
pub const MAX_SIGNATURE_BYTES: u64 = 4096;

const REPOSITORY_URL: &str = "https://github.com/replicraft/TableCraft";

/// An exact release and download pair selected from bounded untrusted metadata.
///
/// Selection does not authenticate the archive. The download must still verify
/// its signature against the compiled-in release key before archive processing.
#[derive(Debug)]
pub struct AvailableUpdate {
    version: Version,
    platform: Platform,
    archive: Asset,
    signature: Asset,
    release_url: String,
}

impl AvailableUpdate {
    /// Returns the strictly newer release version selected from its exact tag.
    pub fn version(&self) -> &Version {
        &self.version
    }

    /// Returns the platform whose archive was selected.
    pub const fn platform(&self) -> Platform {
        self.platform
    }

    /// Returns the exact platform archive filename.
    pub fn archive_name(&self) -> &str {
        &self.archive.name
    }

    /// Returns the exact GitHub release download URL for this archive.
    pub fn archive_url(&self) -> &str {
        &self.archive.browser_download_url
    }

    /// Returns the release's declared archive length, checked against the ceiling.
    pub const fn archive_bytes(&self) -> u64 {
        self.archive.size
    }

    /// Returns the exact GitHub release download URL for the Minisign sidecar.
    pub fn signature_url(&self) -> &str {
        &self.signature.browser_download_url
    }

    /// Returns the release's declared signature length, checked against the ceiling.
    pub const fn signature_bytes(&self) -> u64 {
        self.signature.size
    }

    /// Returns the release's exact canonical GitHub page URL.
    pub fn release_url(&self) -> &str {
        &self.release_url
    }
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    html_url: String,
    assets: Vec<Asset>,
}

#[derive(Debug, Deserialize)]
struct Asset {
    name: String,
    size: u64,
    browser_download_url: String,
}

/// Selects the current platform's exact archive and signature from a GitHub release.
///
/// Build metadata is ignored for version precedence. Stable builds never offer a
/// prerelease, and equal or older releases return `None`. Extra unrelated release
/// assets and unknown JSON fields are tolerated; selected identities are exact.
///
/// # Errors
///
/// Rejects oversized metadata, malformed release identities, inconsistent release
/// flags, missing or repeated selected assets, untrusted URLs and invalid sizes.
/// No filesystem or network operations occur during selection.
pub fn select(
    bytes: &[u8],
    current: &Version,
    platform: Platform,
) -> Result<Option<AvailableUpdate>, UpdateError> {
    if bytes.len() > MAX_METADATA_BYTES {
        return Err(UpdateError::ReleaseTooLarge);
    }

    let release: Release =
        serde_json::from_slice(bytes).map_err(|_| UpdateError::ReleaseMetadata)?;
    let version = release
        .tag_name
        .strip_prefix('v')
        .and_then(|tag| Version::parse(tag).ok())
        .ok_or(UpdateError::ReleaseMetadata)?;
    let release_url = format!("{REPOSITORY_URL}/releases/tag/v{version}");
    if release.tag_name != format!("v{version}")
        || release.draft
        || release.prerelease != !version.pre.is_empty()
        || release.html_url != release_url
    {
        return Err(UpdateError::ReleaseMetadata);
    }

    if version.cmp_precedence(current) != Ordering::Greater
        || (current.pre.is_empty() && !version.pre.is_empty())
    {
        return Ok(None);
    }

    let archive_name = platform.asset_name(&version);
    let signature_name = format!("{archive_name}.minisig");
    let download_prefix = format!("{REPOSITORY_URL}/releases/download/v{version}/");
    let mut archive = None;
    let mut signature = None;

    for asset in release.assets {
        let (selected, maximum) = if asset.name == archive_name {
            (&mut archive, MAX_ARCHIVE_BYTES)
        } else if asset.name == signature_name {
            (&mut signature, MAX_SIGNATURE_BYTES)
        } else {
            continue;
        };

        if selected.is_some()
            || asset.browser_download_url != format!("{download_prefix}{}", asset.name)
        {
            return Err(UpdateError::ReleaseMetadata);
        }
        if asset.size == 0 || asset.size > maximum {
            return Err(UpdateError::AssetTooLarge);
        }
        *selected = Some(asset);
    }

    Ok(Some(AvailableUpdate {
        version,
        platform,
        archive: archive.ok_or(UpdateError::ReleaseMetadata)?,
        signature: signature.ok_or(UpdateError::ReleaseMetadata)?,
        release_url: release.html_url,
    }))
}

#[cfg(test)]
mod tests;

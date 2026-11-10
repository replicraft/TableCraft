use crate::{
    UpdateError,
    http::Transport,
    platform::Platform,
    release::{AvailableUpdate, MAX_SIGNATURE_BYTES},
    verification,
};
use minisign_verify::PublicKey;
use semver::Version;
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};
use tempfile::{NamedTempFile, TempDir};

const HASH_BUFFER_BYTES: usize = 64 * 1024;

/// Byte progress for the archive; signature retrieval precedes archive transfer.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct DownloadProgress {
    /// Authenticated-transfer bytes received so far (not yet an accepted update).
    pub received: u64,
    /// Exact archive size captured from validated release metadata.
    pub total: u64,
}

/// A signature-verified archive with owned private staging and automatic cleanup.
#[derive(Debug)]
pub struct VerifiedDownload {
    // Drop the file before its containing directory, including on Windows.
    file: NamedTempFile,
    _directory: TempDir,
    digest: [u8; 32],
    version: Version,
    archive_name: String,
    size: u64,
    platform: Platform,
}

impl VerifiedDownload {
    pub(crate) fn into_version(self) -> Version {
        self.version
    }

    /// The authenticated version associated with this archive.
    pub fn version(&self) -> &Version {
        &self.version
    }

    /// The exact authenticated platform archive filename.
    pub fn archive_name(&self) -> &str {
        &self.archive_name
    }

    /// The native target authenticated by this archive identity.
    pub fn platform(&self) -> Platform {
        self.platform
    }

    /// Open a stable file handle and recheck its content before installation.
    ///
    /// The consumer must read this returned handle, rather than reopening a path.
    /// This authenticates bytes only; extraction/install policy is a separate gate.
    ///
    /// # Errors
    /// Refuses staging failures, post-download mutation and cancellation.
    pub fn open_verified(&self, cancelled: &dyn Fn() -> bool) -> Result<File, UpdateError> {
        let mut file = self.file.reopen().map_err(UpdateError::Io)?;
        if file.metadata().map_err(UpdateError::Io)?.len() != self.size {
            return Err(UpdateError::PayloadChanged);
        }
        let mut received = 0;
        let mut digest = Sha256::new();
        let mut buffer = [0; HASH_BUFFER_BYTES];
        loop {
            if cancelled() {
                return Err(UpdateError::Cancelled);
            }
            let count = file.read(&mut buffer).map_err(UpdateError::Io)?;
            if count == 0 {
                break;
            }
            received += count as u64;
            if received > self.size {
                return Err(UpdateError::PayloadChanged);
            }
            digest.update(&buffer[..count]);
        }
        let actual: [u8; 32] = digest.finalize().into();
        if actual != self.digest {
            return Err(UpdateError::PayloadChanged);
        }
        file.seek(SeekFrom::Start(0)).map_err(UpdateError::Io)?;
        Ok(file)
    }
}

pub(crate) fn download(
    transport: &dyn Transport,
    key: &PublicKey,
    release: &AvailableUpdate,
    parent: &Path,
    progress: &mut dyn FnMut(DownloadProgress) -> bool,
) -> Result<VerifiedDownload, UpdateError> {
    let total = release.archive_bytes();
    if !progress(DownloadProgress { received: 0, total }) {
        return Err(UpdateError::Cancelled);
    }
    let mut response = transport.get(release.signature_url())?;
    if response
        .length
        .is_some_and(|size| size != release.signature_bytes())
    {
        return Err(UpdateError::LengthMismatch);
    }
    // The bounded signature is fetched before any large archive or disk staging.
    let bytes =
        verification::bounded_bytes(&mut response.reader, MAX_SIGNATURE_BYTES as usize, &|| {
            false
        })?;
    if bytes.len() as u64 != release.signature_bytes() {
        return Err(UpdateError::LengthMismatch);
    }
    let signature = verification::signature(&bytes, release)?;
    // Key-id and prehash admission happen before fetching the payload.
    key.verify_stream(&signature)
        .map_err(|_| UpdateError::Signature)?;
    if !progress(DownloadProgress { received: 0, total }) {
        return Err(UpdateError::Cancelled);
    }

    let directory = tempfile::Builder::new()
        .prefix(".tablecraft-update-")
        .tempdir_in(parent)
        .map_err(UpdateError::Io)?;
    let mut file = NamedTempFile::new_in(directory.path()).map_err(UpdateError::Io)?;
    let mut response = transport.get(release.archive_url())?;
    if response.length.is_some_and(|size| size != total) {
        return Err(UpdateError::LengthMismatch);
    }
    let digest = verification::transfer(
        key,
        &signature,
        &mut response.reader,
        file.as_file_mut(),
        total,
        &mut |received| progress(DownloadProgress { received, total }),
    )?;
    file.as_file().sync_all().map_err(UpdateError::Io)?;

    // Cancellation after authentication still cleans all staging rather than
    // publishing a verified update the caller no longer wants.
    if !progress(DownloadProgress {
        received: total,
        total,
    }) {
        return Err(UpdateError::Cancelled);
    }
    Ok(VerifiedDownload {
        file,
        _directory: directory,
        digest,
        // NO_CLONE: the staged capability owns an immutable version independent
        // of the borrowed release used by the host's progress/status display.
        version: release.version().clone(),
        archive_name: release.archive_name().to_owned(),
        size: total,
        platform: release.platform(),
    })
}

#[cfg(test)]
mod tests;

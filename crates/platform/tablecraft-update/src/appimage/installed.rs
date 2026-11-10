use super::{context::Context, payload};
use crate::{
    UpdateError,
    artifact::{self, ArtifactKind},
    http::Transport,
    installation::{self, InstallPlan, Installed},
    package,
    platform::Platform,
    release::{MAX_ASSET_BYTES, MAX_SIGNATURE_BYTES},
    verification,
};
use minisign_verify::PublicKey;
use semver::Version;
use std::{
    fs::{self, File},
    io,
    path::{Path, PathBuf},
};

const RUNTIME_EXECUTABLE: &str = "usr/bin/tablecraft-desktop";

/// An original AppImage authenticated against its publisher's old-version signature.
///
/// Runtime environment paths merely identify candidates. Qualification also checks
/// the running AppDir's manifest/executable and verifies the complete original
/// image under the compiled publisher key with exact version/platform identity.
#[derive(Debug)]
pub struct InstalledAppImage {
    image: PathBuf,
    directory: PathBuf,
    runtime: Installed,
    digest: [u8; 32],
    length: u64,
}

impl InstalledAppImage {
    /// Returns the original image to replace, outside its temporary runtime tree.
    pub fn image(&self) -> &Path {
        &self.image
    }

    /// Returns the original image's parent for same-filesystem private staging.
    pub fn staging_parent(&self) -> &Path {
        &self.directory
    }

    /// Returns the current version qualified by both runtime and publisher proofs.
    pub fn version(&self) -> &Version {
        self.runtime.version()
    }

    /// Returns the native platform authenticated by the original image identity.
    pub const fn platform(&self) -> Platform {
        self.runtime.platform()
    }

    pub(crate) fn revalidate(&self, cancelled: &dyn Fn() -> bool) -> Result<(), UpdateError> {
        if cancelled() {
            return Err(UpdateError::Cancelled);
        }
        self.runtime.revalidate()?;
        let metadata = fs::symlink_metadata(&self.image).map_err(UpdateError::Io)?;
        if !metadata.is_file() || metadata.len() != self.length {
            return Err(UpdateError::PayloadChanged);
        }
        let digest = package::hash_file(&self.image, MAX_ASSET_BYTES, cancelled)?;
        if digest != Some(self.digest) {
            return Err(UpdateError::PayloadChanged);
        }
        Ok(())
    }
}

pub(crate) fn authenticate(
    transport: &dyn Transport,
    key: &PublicKey,
    context: Context,
    version: &Version,
    platform: Platform,
    cancelled: &dyn Fn() -> bool,
) -> Result<InstalledAppImage, UpdateError> {
    let executable = std::env::current_exe().map_err(UpdateError::Io)?;
    authenticate_at(
        transport,
        key,
        context,
        &executable,
        version,
        platform,
        cancelled,
    )
}

fn authenticate_at(
    transport: &dyn Transport,
    key: &PublicKey,
    context: Context,
    executable: &Path,
    version: &Version,
    platform: Platform,
    cancelled: &dyn Fn() -> bool,
) -> Result<InstalledAppImage, UpdateError> {
    if cancelled() {
        return Err(UpdateError::Cancelled);
    }
    let image = context.image.canonicalize().map_err(UpdateError::Io)?;
    let appdir = context.appdir.canonicalize().map_err(UpdateError::Io)?;
    let executable = executable.canonicalize().map_err(UpdateError::Io)?;
    if !fs::symlink_metadata(&appdir)
        .map_err(UpdateError::Io)?
        .is_dir()
        || image.starts_with(&appdir)
        || appdir
            .join(RUNTIME_EXECUTABLE)
            .canonicalize()
            .map_err(UpdateError::Io)?
            != executable
    {
        return Err(UpdateError::UnqualifiedInstallation);
    }
    let directory = image
        .parent()
        .ok_or(UpdateError::UnqualifiedInstallation)?
        .to_path_buf();
    let runtime = match installation::classify(&executable, platform, version, false)? {
        InstallPlan::Portable(runtime) => runtime,
        InstallPlan::Manual(_) | InstallPlan::MacBundle(_) => {
            return Err(UpdateError::UnqualifiedInstallation);
        }
    };
    let metadata = fs::symlink_metadata(&image).map_err(UpdateError::Io)?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > MAX_ASSET_BYTES {
        return Err(UpdateError::UnqualifiedInstallation);
    }
    let mut file = File::open(&image).map_err(UpdateError::Io)?;
    payload::check(&mut file, platform)?;
    let identity = ArtifactKind::AppImage.file_name(version, platform)?;
    let signature_url = artifact::download_url(version, &format!("{identity}.minisig"));
    let mut response = transport.get(&signature_url)?;
    if response
        .length
        .is_some_and(|length| length > MAX_SIGNATURE_BYTES)
    {
        return Err(UpdateError::AssetTooLarge);
    }
    let text = verification::bounded_bytes(
        &mut response.reader,
        MAX_SIGNATURE_BYTES as usize,
        &mut || cancelled(),
    )?;
    if response
        .length
        .is_some_and(|length| length != text.len() as u64)
    {
        return Err(UpdateError::LengthMismatch);
    }
    let signature = verification::signature(&text, version, &identity)?;
    let digest = verification::transfer(
        key,
        &signature,
        &mut file,
        &mut io::sink(),
        metadata.len(),
        &mut |_| !cancelled(),
    )?;
    let installed = InstalledAppImage {
        image,
        directory,
        runtime,
        digest,
        length: metadata.len(),
    };
    installed.revalidate(cancelled)?;
    Ok(installed)
}

#[cfg(test)]
mod tests;

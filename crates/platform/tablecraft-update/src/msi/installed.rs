use super::{native, paths, report::Report};
use crate::{UpdateError, package, platform::Platform};
use semver::Version;
use serde::Deserialize;
use serde_json::json;
use std::{
    fs,
    path::{Path, PathBuf},
};

pub(super) const MAX_PAYLOAD_BYTES: u64 = 512 * 1024 * 1024;
const MAX_RELATED_PRODUCTS: usize = 16;
const MAX_MANIFEST_BYTES: u64 = 64 * 1024;

/// A Windows Installer registration bound to the running installed executable.
#[derive(Debug)]
pub struct MsiInstallation {
    canonical_executable: PathBuf,
    executable: PathBuf,
    version: Version,
    platform: Platform,
    product: String,
    executable_digest: [u8; 32],
    manifest_digest: [u8; 32],
    staging: PathBuf,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Products {
    products: Vec<Product>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Product {
    product: String,
    executable: PathBuf,
    cached: PathBuf,
    transforms: String,
    package: Report,
}

impl MsiInstallation {
    /// The registered installed version, independently checked against local bytes.
    pub fn version(&self) -> &Version {
        &self.version
    }
    /// The native target confirmed by the cached MSI summary.
    pub const fn platform(&self) -> Platform {
        self.platform
    }
    /// Private user staging; Windows Installer owns the protected destination.
    pub fn staging_parent(&self) -> &Path {
        &self.staging
    }
    pub(super) fn executable(&self) -> &Path {
        // Preserve the native registered spelling for COM/helper comparisons and
        // relaunch. The separately held canonical anchor proves physical identity.
        &self.executable
    }
    pub(super) fn product(&self) -> &str {
        &self.product
    }
    pub(super) fn executable_digest(&self) -> &[u8; 32] {
        &self.executable_digest
    }
    pub(super) fn manifest_digest(&self) -> &[u8; 32] {
        &self.manifest_digest
    }
    pub(super) fn revalidate(&self, cancelled: &dyn Fn() -> bool) -> Result<(), UpdateError> {
        let current = detect(&self.version, self.platform, cancelled)?;
        if current.product != self.product
            || current.canonical_executable != self.canonical_executable
            || current.executable != self.executable
            || current.executable_digest != self.executable_digest
            || current.manifest_digest != self.manifest_digest
        {
            return Err(UpdateError::PayloadChanged);
        }
        Ok(())
    }
}

#[cfg(windows)]
pub(crate) fn hint(platform: Platform) -> Result<bool, UpdateError> {
    if !matches!(platform, Platform::WindowsX64 | Platform::WindowsArm64) {
        return Ok(false);
    }
    let executable = paths::canonicalize(&std::env::current_exe().map_err(UpdateError::Io)?)?;
    let Some(parent) = executable.parent() else {
        return Ok(false);
    };
    let path = parent.join("RELEASE.json");
    let marker = match fs::symlink_metadata(&path) {
        Ok(value) if value.is_file() && value.len() <= MAX_MANIFEST_BYTES => {
            let bytes = super::preparation::read_bounded(&path, MAX_MANIFEST_BYTES)?;
            serde_json::from_slice::<package::Manifest>(&bytes)
                .is_ok_and(|manifest| manifest.installation.is_some())
        }
        Ok(_) => false,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => return Err(UpdateError::Io(error)),
    };
    if marker {
        return Ok(true);
    }
    // A removed/malformed marker must not route an actually registered MSI into
    // portable replacement. This lookup only selects the inspection protocol;
    // detect() still issues authority from the complete native registration.
    let context = native::inspect(
        &json!({"operation":"hint", "executable":executable}),
        &|| false,
    )?;
    context
        .get("registered")
        .and_then(serde_json::Value::as_bool)
        .ok_or(UpdateError::Archive)
}

pub(crate) fn detect(
    version: &Version,
    platform: Platform,
    cancelled: &dyn Fn() -> bool,
) -> Result<MsiInstallation, UpdateError> {
    if !cfg!(windows) || !matches!(platform, Platform::WindowsX64 | Platform::WindowsArm64) {
        return Err(UpdateError::Archive);
    }
    let executable = paths::canonicalize(&std::env::current_exe().map_err(UpdateError::Io)?)?;
    let report = native::inspect(&json!({"operation": "detect"}), cancelled)?;
    let products: Products = serde_json::from_value(report).map_err(|_| UpdateError::Archive)?;
    seal_registered(executable, version, platform, products, cancelled)
}

pub(super) fn seal_registered(
    executable: PathBuf,
    version: &Version,
    platform: Platform,
    products: Products,
    cancelled: &dyn Fn() -> bool,
) -> Result<MsiInstallation, UpdateError> {
    if products.products.len() > MAX_RELATED_PRODUCTS {
        return Err(UpdateError::Archive);
    }
    let mut selected = None;
    for product in products.products {
        if paths::canonicalize(&product.executable)? != executable {
            continue;
        }
        if selected.is_some()
            || !product.transforms.is_empty()
            || product.package.validate(version, platform)? != product.product
            || !product.cached.is_absolute()
            || !fs::symlink_metadata(&product.cached)
                .map_err(UpdateError::Io)?
                .is_file()
        {
            return Err(UpdateError::Archive);
        }
        selected = Some(product);
    }
    let product = selected.ok_or(UpdateError::Archive)?;
    paths::ensure_compatible(&product.executable)?;
    let parent = executable.parent().ok_or(UpdateError::Archive)?;
    for file in &product.package.files {
        let name = file
            .name
            .split('|')
            .next_back()
            .ok_or(UpdateError::Archive)?;
        let metadata = fs::symlink_metadata(parent.join(name)).map_err(UpdateError::Io)?;
        if !metadata.is_file() || metadata.len().to_string() != file.size {
            return Err(UpdateError::PayloadChanged);
        }
    }
    let manifest = parent.join("RELEASE.json");
    let bytes = super::preparation::read_bounded(&manifest, MAX_MANIFEST_BYTES)?;
    if bytes.len() as u64 > MAX_MANIFEST_BYTES
        || !fs::symlink_metadata(&manifest)
            .map_err(UpdateError::Io)?
            .is_file()
    {
        return Err(UpdateError::Archive);
    }
    let metadata: package::Manifest =
        serde_json::from_slice(&bytes).map_err(|_| UpdateError::Archive)?;
    if metadata.installation.is_none() || !metadata.matches_installation(platform, version) {
        return Err(UpdateError::Archive);
    }
    let mut executable_digest = None;
    for (name, expected) in &metadata.files {
        let actual = package::hash_file(&parent.join(name), MAX_PAYLOAD_BYTES, cancelled)?
            .ok_or(UpdateError::Archive)?;
        if !package::hash_matches(expected, &actual) {
            return Err(UpdateError::PayloadChanged);
        }
        if name == "tablecraft-desktop.exe" {
            executable_digest = Some(actual);
        }
    }
    let executable_digest = executable_digest.ok_or(UpdateError::Archive)?;
    let manifest_digest = package::hash_file(&manifest, MAX_MANIFEST_BYTES, cancelled)?
        .ok_or(UpdateError::Archive)?;
    let staging = paths::canonicalize(&std::env::temp_dir())?;
    Ok(MsiInstallation {
        canonical_executable: executable,
        executable: product.executable,
        // NO_CLONE: the sealed installation outlives its caller's borrowed
        // running version and must own the immutable native identity.
        version: version.clone(),
        platform,
        product: product.product,
        executable_digest,
        manifest_digest,
        staging,
    })
}

//! Installation ownership checks independent of release-controlled filenames.

use crate::{UpdateError, platform::Platform};
use semver::Version;
use serde::{
    Deserialize, Deserializer,
    de::{self, MapAccess, Visitor},
};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fmt,
    fs::{self, File},
    io::{self, Read},
    path::{Path, PathBuf},
};

// Installed manifests contain at most five fixed file entries. This independent
// ceiling bounds metadata parsing before any claimed payload paths are considered.
const MAX_MANIFEST_BYTES: u64 = 64 * 1024;
const MAX_EXECUTABLE_BYTES: u64 = 512 * 1024 * 1024;
const HASH_BUFFER_BYTES: usize = 64 * 1024;
const SHA256_HEX_BYTES: usize = 2 * 32;
const HEX_DIGITS: &[u8; 16] = b"0123456789abcdef";
const FLAT_FILES: [&str; 3] = ["tablecraft-desktop", "LICENSE", "ThirdPartyLicenses.txt"];
const WINDOWS_FILES: [&str; 3] = [
    "tablecraft-desktop.exe",
    "LICENSE",
    "ThirdPartyLicenses.txt",
];
const MAC_FILES: [&str; 5] = [
    "TableCraft.app/Contents/MacOS/tablecraft-desktop",
    "TableCraft.app/Contents/Info.plist",
    "TableCraft.app/Contents/Resources/LICENSE",
    "TableCraft.app/Contents/Resources/ThirdPartyLicenses.txt",
    "TableCraft.app/Contents/Resources/TableCraft.icns",
];

/// Why this installation must use a manual update instead of replacement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManualReason {
    /// Debug binaries and Cargo output trees are not installed release packages.
    DevelopmentBuild,
    /// The executable does not have a supported native package layout.
    UnrecognizedLayout,
    /// The package-owned release manifest is absent.
    MissingManifest,
    /// Package identity, membership or the running executable hash differs.
    InvalidManifest,
    /// The executable belongs to a location managed outside portable updates.
    UnsupportedLocation,
}

/// A detected manual installation or a sealed package-owned installation.
#[derive(Debug)]
pub enum InstallPlan {
    /// Show the release page instead of attempting installation replacement.
    Manual(ManualReason),
    /// A manifest-backed Windows or Linux portable directory.
    Portable(Installed),
    /// A manifest-backed native macOS application bundle.
    MacBundle(Installed),
}

/// Immutable local anchors and content witnesses for an installed desktop package.
///
/// The paths come from the running executable, never from release metadata.
/// Detection does not establish parent-directory write permission or authorize
/// replacing files while a document or save worker remains active.
#[derive(Debug)]
pub struct Installed {
    executable: PathBuf,
    directory: PathBuf,
    manifest: PathBuf,
    bundle: Option<PathBuf>,
    platform: Platform,
    version: Version,
    executable_digest: [u8; 32],
    manifest_digest: [u8; 32],
}

impl Installed {
    /// Returns the canonical running executable path.
    pub fn executable(&self) -> &Path {
        &self.executable
    }

    /// Returns the same-filesystem staging parent for this installation.
    ///
    /// This is the executable's directory for portable packages and the bundle's
    /// parent directory for macOS applications.
    pub fn directory(&self) -> &Path {
        &self.directory
    }

    /// Returns the package-owned manifest path.
    pub fn manifest(&self) -> &Path {
        &self.manifest
    }

    /// Returns the application bundle anchor for macOS packages.
    pub fn bundle(&self) -> Option<&Path> {
        self.bundle.as_deref()
    }

    /// Returns the native platform captured from the installed manifest.
    pub const fn platform(&self) -> Platform {
        self.platform
    }

    /// Returns the running version confirmed against the installed manifest.
    pub fn version(&self) -> &Version {
        &self.version
    }

    /// Rechecks local identity and content after an update or confirmation wait.
    ///
    /// # Errors
    /// Refuses changed anchors, package metadata or executable contents. Filesystem
    /// failures remain distinct from a changed installation. This does not grant
    /// write permission or remove the need for atomic installation operations.
    pub fn revalidate(&self) -> Result<(), UpdateError> {
        let plan = classify(&self.executable, self.platform, &self.version, false)?;
        let current = match plan {
            InstallPlan::Portable(current) | InstallPlan::MacBundle(current) => current,
            InstallPlan::Manual(_) => return Err(UpdateError::PayloadChanged),
        };
        if current.executable != self.executable
            || current.directory != self.directory
            || current.bundle != self.bundle
            || current.manifest != self.manifest
            || current.executable_digest != self.executable_digest
            || current.manifest_digest != self.manifest_digest
        {
            return Err(UpdateError::PayloadChanged);
        }
        Ok(())
    }
}

/// Detects installation ownership from the running executable and its package manifest.
///
/// # Errors
/// Reports current-executable discovery and local filesystem failures. Unknown,
/// development, package-managed and malformed installations return a manual plan.
/// This blocking operation includes hashing the executable and belongs on a worker.
pub fn detect(current: &Version, platform: Platform) -> Result<InstallPlan, UpdateError> {
    let executable = std::env::current_exe().map_err(UpdateError::Io)?;
    classify(&executable, platform, current, cfg!(debug_assertions))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    product: String,
    version: String,
    target: String,
    #[serde(deserialize_with = "file_hashes")]
    files: BTreeMap<String, String>,
}

fn classify(
    executable: &Path,
    platform: Platform,
    current: &Version,
    debug_build: bool,
) -> Result<InstallPlan, UpdateError> {
    if debug_build || has_cargo_ancestor(executable)? {
        return Ok(InstallPlan::Manual(ManualReason::DevelopmentBuild));
    }
    if !executable.is_absolute() || !regular(executable)? {
        return Ok(InstallPlan::Manual(ManualReason::UnrecognizedLayout));
    }

    let executable = executable.canonicalize().map_err(UpdateError::Io)?;
    if has_cargo_ancestor(&executable)? {
        return Ok(InstallPlan::Manual(ManualReason::DevelopmentBuild));
    }
    if managed_location(&executable, platform) {
        return Ok(InstallPlan::Manual(ManualReason::UnsupportedLocation));
    }
    let Some(parent) = executable.parent() else {
        return Ok(InstallPlan::Manual(ManualReason::UnrecognizedLayout));
    };
    let mac = matches!(platform, Platform::MacosX64 | Platform::MacosArm64);
    let windows = matches!(platform, Platform::WindowsX64 | Platform::WindowsArm64);
    let expected_executable = if windows {
        "tablecraft-desktop.exe"
    } else {
        "tablecraft-desktop"
    };
    if executable.file_name().and_then(|name| name.to_str()) != Some(expected_executable) {
        return Ok(InstallPlan::Manual(ManualReason::UnrecognizedLayout));
    }

    let (directory, bundle, manifest) = if mac {
        let Some(contents) = parent.parent() else {
            return Ok(InstallPlan::Manual(ManualReason::UnrecognizedLayout));
        };
        let Some(bundle) = contents.parent() else {
            return Ok(InstallPlan::Manual(ManualReason::UnrecognizedLayout));
        };
        let Some(directory) = bundle.parent() else {
            return Ok(InstallPlan::Manual(ManualReason::UnrecognizedLayout));
        };
        if parent.file_name().and_then(|name| name.to_str()) != Some("MacOS")
            || contents.file_name().and_then(|name| name.to_str()) != Some("Contents")
            || bundle.extension().and_then(|name| name.to_str()) != Some("app")
        {
            return Ok(InstallPlan::Manual(ManualReason::UnrecognizedLayout));
        }
        (
            directory.to_path_buf(),
            Some(bundle.to_path_buf()),
            contents.join("Resources/RELEASE.json"),
        )
    } else {
        (parent.to_path_buf(), None, parent.join("RELEASE.json"))
    };

    let manifest_metadata = match fs::symlink_metadata(&manifest) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(InstallPlan::Manual(ManualReason::MissingManifest));
        }
        Err(error) => return Err(UpdateError::Io(error)),
    };
    if !manifest_metadata.is_file() || manifest_metadata.len() > MAX_MANIFEST_BYTES {
        return Ok(InstallPlan::Manual(ManualReason::InvalidManifest));
    }
    let mut bytes = Vec::new();
    File::open(&manifest)
        .map_err(UpdateError::Io)?
        .take(MAX_MANIFEST_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(UpdateError::Io)?;
    if bytes.len() as u64 > MAX_MANIFEST_BYTES {
        return Ok(InstallPlan::Manual(ManualReason::InvalidManifest));
    }
    let Ok(metadata) = serde_json::from_slice::<Manifest>(&bytes) else {
        return Ok(InstallPlan::Manual(ManualReason::InvalidManifest));
    };
    let Ok(version) = Version::parse(&metadata.version) else {
        return Ok(InstallPlan::Manual(ManualReason::InvalidManifest));
    };
    let expected_files = if mac {
        MAC_FILES.as_slice()
    } else if windows {
        WINDOWS_FILES.as_slice()
    } else {
        FLAT_FILES.as_slice()
    };
    if metadata.product != "tablecraft-desktop"
        || metadata.target != platform.target()
        || version != *current
        || metadata.files.len() != expected_files.len()
        || !expected_files.iter().all(|name| {
            metadata
                .files
                .get(*name)
                .is_some_and(|hash| valid_hash(hash))
        })
    {
        return Ok(InstallPlan::Manual(ManualReason::InvalidManifest));
    }

    if mac {
        let Some(resources) = manifest.parent() else {
            return Ok(InstallPlan::Manual(ManualReason::InvalidManifest));
        };
        if !fs::symlink_metadata(resources)
            .map_err(UpdateError::Io)?
            .is_dir()
        {
            return Ok(InstallPlan::Manual(ManualReason::InvalidManifest));
        }
    }
    for name in expected_files {
        let path = if let Some(bundle) = &bundle {
            let Some(relative) = name.strip_prefix("TableCraft.app/") else {
                return Ok(InstallPlan::Manual(ManualReason::InvalidManifest));
            };
            bundle.join(relative)
        } else {
            directory.join(name)
        };
        if !regular(&path)? {
            return Ok(InstallPlan::Manual(ManualReason::InvalidManifest));
        }
    }

    let expected_hash = &metadata.files[expected_files[0]];
    let Some(executable_digest) = hash_executable(&executable)? else {
        return Ok(InstallPlan::Manual(ManualReason::InvalidManifest));
    };
    if !hash_matches(expected_hash, &executable_digest) {
        return Ok(InstallPlan::Manual(ManualReason::InvalidManifest));
    }
    let installed = Installed {
        executable,
        directory,
        manifest,
        bundle,
        platform,
        version,
        executable_digest,
        manifest_digest: Sha256::digest(bytes).into(),
    };
    Ok(if mac {
        InstallPlan::MacBundle(installed)
    } else {
        InstallPlan::Portable(installed)
    })
}

fn regular(path: &Path) -> Result<bool, UpdateError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => Ok(metadata.is_file()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(UpdateError::Io(error)),
    }
}

fn has_cargo_ancestor(executable: &Path) -> Result<bool, UpdateError> {
    for parent in executable.ancestors().skip(1) {
        if regular(&parent.join("CACHEDIR.TAG"))? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn managed_location(executable: &Path, platform: Platform) -> bool {
    match platform {
        Platform::LinuxX64 | Platform::LinuxArm64 => {
            ["/usr", "/bin", "/sbin", "/snap", "/app", "/var/lib/flatpak"]
                .iter()
                .any(|root| executable.starts_with(root))
        }
        Platform::WindowsX64 | Platform::WindowsArm64 => executable.components().any(|part| {
            let Some(part) = part.as_os_str().to_str() else {
                return false;
            };
            part.eq_ignore_ascii_case("Program Files")
                || part.eq_ignore_ascii_case("Program Files (x86)")
        }),
        Platform::MacosX64 | Platform::MacosArm64 => false,
    }
}

fn valid_hash(hash: &str) -> bool {
    hash.len() == SHA256_HEX_BYTES
        && hash
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn hash_matches(expected: &str, actual: &[u8; 32]) -> bool {
    expected
        .as_bytes()
        .chunks_exact(2)
        .zip(actual)
        .all(|(digits, byte)| {
            digits[0] == HEX_DIGITS[usize::from(byte >> 4)]
                && digits[1] == HEX_DIGITS[usize::from(byte & 0x0f)]
        })
}

fn hash_executable(path: &Path) -> Result<Option<[u8; 32]>, UpdateError> {
    let mut file = File::open(path).map_err(UpdateError::Io)?;
    let size = file.metadata().map_err(UpdateError::Io)?.len();
    if size == 0 || size > MAX_EXECUTABLE_BYTES {
        return Ok(None);
    }
    let mut digest = Sha256::new();
    let mut buffer = [0; HASH_BUFFER_BYTES];
    let mut received = 0;
    loop {
        let count = file.read(&mut buffer).map_err(UpdateError::Io)?;
        if count == 0 {
            break;
        }
        received += count as u64;
        if received > size {
            return Ok(None);
        }
        digest.update(&buffer[..count]);
    }
    if received != size {
        return Ok(None);
    }
    Ok(Some(digest.finalize().into()))
}

fn file_hashes<'de, D>(deserializer: D) -> Result<BTreeMap<String, String>, D::Error>
where
    D: Deserializer<'de>,
{
    struct Hashes;

    impl<'de> Visitor<'de> for Hashes {
        type Value = BTreeMap<String, String>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("unique installed file hashes")
        }

        fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
        where
            M: MapAccess<'de>,
        {
            let mut files = BTreeMap::new();
            while let Some((name, hash)) = map.next_entry::<String, String>()? {
                if files.insert(name, hash).is_some() {
                    return Err(de::Error::custom("duplicate installed file hash"));
                }
            }
            Ok(files)
        }
    }

    deserializer.deserialize_map(Hashes)
}

#[cfg(test)]
mod tests;

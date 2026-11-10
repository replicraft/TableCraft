//! One package identity, payload inventory and strict release-manifest contract.

use crate::{UpdateError, platform::Platform};
use semver::Version;
use serde::{
    Deserialize, Deserializer,
    de::{self, MapAccess, Visitor},
};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    fs::{self, File},
    io::Read,
    path::Path,
};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

pub(crate) const MAC_BUNDLE: &str = "TableCraft.app";
pub(crate) const MAC_PLIST: &str = "TableCraft.app/Contents/Info.plist";
const MAC_EXECUTABLE: &str = "TableCraft.app/Contents/MacOS/tablecraft-desktop";
const MAC_MANIFEST: &str = "TableCraft.app/Contents/Resources/RELEASE.json";
const MAC_FILES: [&str; 6] = [
    MAC_EXECUTABLE,
    "TableCraft.app/Contents/Resources/LICENSE",
    "TableCraft.app/Contents/Resources/ThirdPartyLicenses.txt",
    MAC_PLIST,
    "TableCraft.app/Contents/Resources/TableCraft.icns",
    MAC_MANIFEST,
];
const WINDOWS_FILES: [&str; 4] = [
    "tablecraft-desktop.exe",
    "LICENSE",
    "ThirdPartyLicenses.txt",
    "RELEASE.json",
];
const LINUX_FILES: [&str; 4] = [
    "tablecraft-desktop",
    "LICENSE",
    "ThirdPartyLicenses.txt",
    "RELEASE.json",
];
const MAC_DIRECTORIES: [&str; 4] = [
    MAC_BUNDLE,
    "TableCraft.app/Contents",
    "TableCraft.app/Contents/MacOS",
    "TableCraft.app/Contents/Resources",
];
const HASH_BUFFER_BYTES: usize = 64 * 1024;
const SHA256_HEX_BYTES: usize = 2 * 32;
const HEX_DIGITS: &[u8; 16] = b"0123456789abcdef";
const NIBBLE_BITS: u32 = 4;
const NIBBLE_MASK: u8 = 0x0f;
#[cfg(unix)]
const PERMISSION_MASK: u32 = 0o7777;
#[cfg(unix)]
const EXECUTABLE_MODE: u32 = 0o755;
#[cfg(unix)]
const DATA_MODE: u32 = 0o644;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Manifest {
    product: String,
    pub(crate) version: String,
    target: String,
    #[serde(deserialize_with = "unique_hashes")]
    pub(crate) files: BTreeMap<String, String>,
}

impl Manifest {
    pub(crate) fn matches_release(&self, platform: Platform, version: &Version) -> bool {
        let expected = files(platform);
        let manifest = manifest(platform);
        self.product == "tablecraft-desktop"
            && self.version == version.to_string()
            && self.target == platform.target()
            && self.files.len() + 1 == expected.len()
            && !self.files.contains_key(manifest)
            && expected
                .iter()
                .filter(|name| **name != manifest)
                .all(|name| self.files.get(*name).is_some_and(|hash| valid_hash(hash)))
    }
}

pub(crate) fn is_mac(platform: Platform) -> bool {
    matches!(platform, Platform::MacosX64 | Platform::MacosArm64)
}

pub(crate) fn files(platform: Platform) -> &'static [&'static str] {
    match platform {
        Platform::MacosX64 | Platform::MacosArm64 => &MAC_FILES,
        Platform::WindowsX64 | Platform::WindowsArm64 => &WINDOWS_FILES,
        Platform::LinuxX64 | Platform::LinuxArm64 => &LINUX_FILES,
    }
}

pub(crate) fn directories(platform: Platform) -> &'static [&'static str] {
    if is_mac(platform) {
        &MAC_DIRECTORIES
    } else {
        &[]
    }
}

pub(crate) fn executable(platform: Platform) -> &'static str {
    files(platform)[0]
}

pub(crate) fn manifest(platform: Platform) -> &'static str {
    if is_mac(platform) {
        MAC_MANIFEST
    } else {
        "RELEASE.json"
    }
}

/// Consumers retain independent metadata and operation ceilings at their boundary.
pub(crate) struct Limits {
    pub(crate) executable: u64,
    pub(crate) legal: u64,
    pub(crate) icon: u64,
    pub(crate) metadata: u64,
    pub(crate) total: u64,
}

impl Limits {
    pub(crate) fn maximum(&self, name: &str, platform: Platform) -> u64 {
        if name == executable(platform) {
            self.executable
        } else if name.ends_with(".json") || name.ends_with(".plist") {
            self.metadata
        } else if name.ends_with(".icns") {
            self.icon
        } else {
            self.legal
        }
    }
}

/// Recheck the sealed staging tree after a confirmation wait, including its
/// original manifest witness so an attacker cannot simply rewrite all hashes.
pub(crate) fn validate_directory(
    root: &Path,
    platform: Platform,
    version: &Version,
    manifest_digest: &[u8; 32],
    limits: &Limits,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), UpdateError> {
    census(root, platform, cancelled)?;
    check_cancelled(cancelled)?;
    let path = root.join(manifest(platform));
    let size = fs::metadata(&path).map_err(UpdateError::Io)?.len();
    if size == 0 || size > limits.metadata {
        return Err(UpdateError::PayloadChanged);
    }
    let mut bytes = Vec::new();
    File::open(&path)
        .map_err(UpdateError::Io)?
        .take(limits.metadata + 1)
        .read_to_end(&mut bytes)
        .map_err(UpdateError::Io)?;
    let actual: [u8; 32] = Sha256::digest(&bytes).into();
    if bytes.len() as u64 > limits.metadata || actual != *manifest_digest {
        return Err(UpdateError::PayloadChanged);
    }
    let metadata: Manifest =
        serde_json::from_slice(&bytes).map_err(|_| UpdateError::PayloadChanged)?;
    if !metadata.matches_release(platform, version) {
        return Err(UpdateError::PayloadChanged);
    }
    let mut total = bytes.len() as u64;
    for (name, expected) in &metadata.files {
        check_cancelled(cancelled)?;
        let path = root.join(name);
        let size = fs::metadata(&path).map_err(UpdateError::Io)?.len();
        total = total.checked_add(size).ok_or(UpdateError::PayloadChanged)?;
        if total > limits.total {
            return Err(UpdateError::PayloadChanged);
        }
        let Some(actual) = hash_file(&path, limits.maximum(name, platform), cancelled)? else {
            return Err(UpdateError::PayloadChanged);
        };
        if !hash_matches(expected, &actual) {
            return Err(UpdateError::PayloadChanged);
        }
    }
    Ok(())
}

fn census(
    root: &Path,
    platform: Platform,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), UpdateError> {
    if !fs::symlink_metadata(root)
        .map_err(UpdateError::Io)?
        .is_dir()
    {
        return Err(UpdateError::PayloadChanged);
    }
    let directories = directories(platform);
    let expected = files(platform);
    let mut seen_files = BTreeSet::new();
    let mut seen_directories = BTreeSet::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory).map_err(UpdateError::Io)? {
            check_cancelled(cancelled)?;
            let entry = entry.map_err(UpdateError::Io)?;
            let path = entry.path();
            let relative = path
                .strip_prefix(root)
                .map_err(|_| UpdateError::PayloadChanged)?;
            let metadata = fs::symlink_metadata(&path).map_err(UpdateError::Io)?;
            if metadata.is_dir() {
                let Some(name) = directories
                    .iter()
                    .copied()
                    .find(|name| relative == Path::new(name))
                else {
                    return Err(UpdateError::PayloadChanged);
                };
                if !seen_directories.insert(name) {
                    return Err(UpdateError::PayloadChanged);
                }
                pending.push(path);
            } else if metadata.is_file() {
                let Some(name) = expected
                    .iter()
                    .copied()
                    .find(|name| relative == Path::new(name))
                else {
                    return Err(UpdateError::PayloadChanged);
                };
                if !seen_files.insert(name) {
                    return Err(UpdateError::PayloadChanged);
                }
                #[cfg(unix)]
                {
                    let expected_mode = if name == executable(platform) {
                        EXECUTABLE_MODE
                    } else {
                        DATA_MODE
                    };
                    if metadata.permissions().mode() & PERMISSION_MASK != expected_mode {
                        return Err(UpdateError::PayloadChanged);
                    }
                }
            } else {
                return Err(UpdateError::PayloadChanged);
            }
        }
    }
    if seen_files.len() != expected.len() || seen_directories.len() != directories.len() {
        return Err(UpdateError::PayloadChanged);
    }
    Ok(())
}

pub(crate) fn hash_file(
    path: &Path,
    maximum: u64,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<[u8; 32]>, UpdateError> {
    let mut file = File::open(path).map_err(UpdateError::Io)?;
    let metadata = file.metadata().map_err(UpdateError::Io)?;
    let size = metadata.len();
    if !metadata.is_file() || size == 0 || size > maximum {
        return Ok(None);
    }
    let mut digest = Sha256::new();
    let mut received = 0;
    let mut buffer = [0; HASH_BUFFER_BYTES];
    loop {
        check_cancelled(cancelled)?;
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

pub(crate) fn hash_text(hash: &[u8; 32]) -> String {
    let mut text = String::with_capacity(SHA256_HEX_BYTES);
    for byte in hash {
        text.push(char::from(HEX_DIGITS[usize::from(byte >> NIBBLE_BITS)]));
        text.push(char::from(HEX_DIGITS[usize::from(byte & NIBBLE_MASK)]));
    }
    text
}

pub(crate) fn hash_matches(expected: &str, actual: &[u8; 32]) -> bool {
    valid_hash(expected)
        && expected
            .as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .zip(actual)
            .all(|(digits, byte)| {
                digits[0] == HEX_DIGITS[usize::from(byte >> NIBBLE_BITS)]
                    && digits[1] == HEX_DIGITS[usize::from(byte & NIBBLE_MASK)]
            })
}

fn valid_hash(hash: &str) -> bool {
    hash.len() == SHA256_HEX_BYTES
        && hash
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn unique_hashes<'de, D>(deserializer: D) -> Result<BTreeMap<String, String>, D::Error>
where
    D: Deserializer<'de>,
{
    struct Hashes;
    impl<'de> Visitor<'de> for Hashes {
        type Value = BTreeMap<String, String>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("unique package payload hashes")
        }

        fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
        where
            M: MapAccess<'de>,
        {
            let mut files = BTreeMap::new();
            while let Some((name, hash)) = map.next_entry::<String, String>()? {
                if files.insert(name, hash).is_some() {
                    return Err(de::Error::custom("duplicate package payload hash"));
                }
            }
            Ok(files)
        }
    }
    deserializer.deserialize_map(Hashes)
}

fn check_cancelled(cancelled: &dyn Fn() -> bool) -> Result<(), UpdateError> {
    if cancelled() {
        Err(UpdateError::Cancelled)
    } else {
        Ok(())
    }
}

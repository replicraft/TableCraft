//! Bounded extraction of authenticated, exactly shaped desktop release archives.

use crate::{UpdateError, platform::Platform};
use flate2::bufread::GzDecoder;
use semver::Version;
use serde::{
    Deserialize, Deserializer,
    de::{self, MapAccess, Visitor},
};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fmt,
    fs::{self, File, OpenOptions},
    io::{BufReader, Read, Seek, SeekFrom, Write},
    ops::Range,
    path::{Path, PathBuf},
};
use tempfile::TempDir;
use zip::ZipArchive;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

const MAX_OUTPUT_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_EXECUTABLE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_LEGAL_BYTES: u64 = 16 * 1024 * 1024;
const MAX_ICON_BYTES: u64 = 16 * 1024 * 1024;
const MAX_METADATA_BYTES: u64 = 1024 * 1024;
const COPY_BYTES: usize = 64 * 1024;
const ZIP_END_BYTES: usize = 22;
const ZIP_END_SIGNATURE: &[u8; 4] = b"PK\x05\x06";
const ZIP_DISK_FIELDS: Range<usize> = 4..8;
const ZIP_DISK_COUNT_OFFSET: usize = 8;
const ZIP_TOTAL_COUNT_OFFSET: usize = 10;
const ZIP_DIRECTORY_SIZE_OFFSET: usize = 12;
const ZIP_DIRECTORY_START_OFFSET: usize = 16;
const ZIP_COMMENT_LENGTH_OFFSET: usize = 20;
const MAX_ZIP_DIRECTORY_BYTES: u32 = 64 * 1024;
const FILE_TYPE_MASK: u32 = 0o170000;
const REGULAR_FILE_TYPE: u32 = 0o100000;
const PERMISSION_MASK: u32 = 0o7777;
const EXECUTABLE_MODE: u32 = 0o755;
const DATA_MODE: u32 = 0o644;
const MAC_EXECUTABLE: &str = "TableCraft.app/Contents/MacOS/tablecraft-desktop";
const MAC_MANIFEST: &str = "TableCraft.app/Contents/Resources/RELEASE.json";
const MAC_PLIST: &str = "TableCraft.app/Contents/Info.plist";
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

/// An owned private staging tree, not an installed application.
#[derive(Debug)]
pub(crate) struct ExtractedUpdate {
    directory: TempDir,
    executable: PathBuf,
    bundle: Option<PathBuf>,
}

impl ExtractedUpdate {
    pub(crate) fn root(&self) -> &Path {
        self.directory.path()
    }

    pub(crate) fn executable(&self) -> &Path {
        &self.executable
    }

    pub(crate) fn bundle(&self) -> Option<&Path> {
        self.bundle.as_deref()
    }
}

/// Extract only an already authenticated file handle supplied by the update owner.
/// No path-based entry point allows callers to bypass signature verification.
pub(crate) fn extract(
    reader: File,
    platform: Platform,
    version: &Version,
    parent: &Path,
    cancelled: &dyn Fn() -> bool,
) -> Result<ExtractedUpdate, UpdateError> {
    check_cancelled(cancelled)?;
    let expected = expected_files(platform);
    let directory = tempfile::Builder::new()
        .prefix(".tablecraft-extracted-")
        .tempdir_in(parent)
        .map_err(UpdateError::Io)?;
    let mut extracted = Contents {
        root: directory.path(),
        expected,
        hashes: BTreeMap::new(),
        written: 0,
        cancelled,
    };

    match platform {
        Platform::LinuxX64 | Platform::LinuxArm64 => extract_tar(reader, &mut extracted)?,
        _ => extract_zip(reader, &mut extracted)?,
    }
    extracted.validate(platform, version)?;
    check_cancelled(cancelled)?;

    Ok(ExtractedUpdate {
        executable: directory.path().join(expected[0]),
        bundle: is_mac(platform).then(|| directory.path().join("TableCraft.app")),
        directory,
    })
}

fn is_mac(platform: Platform) -> bool {
    matches!(platform, Platform::MacosX64 | Platform::MacosArm64)
}

fn expected_files(platform: Platform) -> &'static [&'static str] {
    match platform {
        Platform::MacosX64 | Platform::MacosArm64 => &MAC_FILES,
        Platform::WindowsX64 | Platform::WindowsArm64 => &WINDOWS_FILES,
        Platform::LinuxX64 | Platform::LinuxArm64 => &LINUX_FILES,
    }
}

struct Contents<'a> {
    root: &'a Path,
    expected: &'static [&'static str],
    hashes: BTreeMap<&'static str, String>,
    written: u64,
    cancelled: &'a dyn Fn() -> bool,
}

impl Contents<'_> {
    fn file(
        &mut self,
        name: &str,
        size: u64,
        mode: u32,
        reader: &mut dyn Read,
    ) -> Result<(), UpdateError> {
        check_cancelled(self.cancelled)?;
        // Only these static names reach join/create_new. This rejects absolute,
        // traversal, separators, reserved names and aliases before filesystem use.
        let name = self
            .expected
            .iter()
            .copied()
            .find(|expected| *expected == name)
            .ok_or(UpdateError::Archive)?;
        if self.hashes.contains_key(name) {
            return Err(UpdateError::Archive);
        }
        let executable = name == self.expected[0];
        let expected_mode = if executable {
            EXECUTABLE_MODE
        } else {
            DATA_MODE
        };
        let maximum = if executable {
            MAX_EXECUTABLE_BYTES
        } else if name.ends_with(".json") || name.ends_with(".plist") {
            MAX_METADATA_BYTES
        } else if name.ends_with(".icns") {
            MAX_ICON_BYTES
        } else {
            MAX_LEGAL_BYTES
        };
        if size == 0 || size > maximum || mode != expected_mode {
            return Err(UpdateError::Archive);
        }
        let destination = self.root.join(name);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(UpdateError::Io)?;
        }
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&destination)
            .map_err(UpdateError::Io)?;
        let mut hash = Sha256::new();
        let mut received = 0;
        let mut buffer = [0; COPY_BYTES];
        loop {
            check_cancelled(self.cancelled)?;
            let allowance = usize::try_from((size - received).min(COPY_BYTES as u64 - 1) + 1)
                .map_err(|_| UpdateError::Archive)?;
            let count = reader
                .read(&mut buffer[..allowance])
                .map_err(|_| UpdateError::Archive)?;
            if count == 0 {
                break;
            }
            received += count as u64;
            self.written += count as u64;
            if received > size || self.written > MAX_OUTPUT_BYTES {
                return Err(UpdateError::Archive);
            }
            output
                .write_all(&buffer[..count])
                .map_err(UpdateError::Io)?;
            hash.update(&buffer[..count]);
        }
        if received != size {
            return Err(UpdateError::Archive);
        }
        output.sync_all().map_err(UpdateError::Io)?;
        #[cfg(unix)]
        fs::set_permissions(&destination, fs::Permissions::from_mode(expected_mode))
            .map_err(UpdateError::Io)?;
        self.hashes.insert(name, format!("{:x}", hash.finalize()));
        Ok(())
    }

    fn validate(&self, platform: Platform, version: &Version) -> Result<(), UpdateError> {
        if self.hashes.len() != self.expected.len() {
            return Err(UpdateError::Archive);
        }
        let manifest_name = if is_mac(platform) {
            MAC_MANIFEST
        } else {
            "RELEASE.json"
        };
        let bytes = fs::read(self.root.join(manifest_name)).map_err(UpdateError::Io)?;
        let manifest: Manifest =
            serde_json::from_slice(&bytes).map_err(|_| UpdateError::Archive)?;
        if manifest.product != "tablecraft-desktop"
            || manifest.version != version.to_string()
            || manifest.target != platform.target()
            || manifest.files.len() + 1 != self.hashes.len()
            || manifest.files.contains_key(manifest_name)
            || !manifest
                .files
                .iter()
                .all(|(name, hash)| self.hashes.get(name.as_str()) == Some(hash))
        {
            return Err(UpdateError::Archive);
        }
        if is_mac(platform) {
            let actual = fs::read(self.root.join(MAC_PLIST)).map_err(UpdateError::Io)?;
            if actual != canonical_plist(version).as_bytes() {
                return Err(UpdateError::Archive);
            }
        }
        Ok(())
    }
}

fn extract_zip(mut reader: File, output: &mut Contents<'_>) -> Result<(), UpdateError> {
    // The controlled packager emits a comment-free, non-spanned EOCD, with only
    // four or six central entries. Bound this before ZipArchive allocates metadata.
    let length = reader.metadata().map_err(UpdateError::Io)?.len();
    if length < ZIP_END_BYTES as u64 {
        return Err(UpdateError::Archive);
    }
    reader
        .seek(SeekFrom::End(-(ZIP_END_BYTES as i64)))
        .map_err(UpdateError::Io)?;
    let mut end = [0; ZIP_END_BYTES];
    reader
        .read_exact(&mut end)
        .map_err(|_| UpdateError::Archive)?;
    let count = u16_at(&end, ZIP_TOTAL_COUNT_OFFSET);
    let directory_size = u32_at(&end, ZIP_DIRECTORY_SIZE_OFFSET);
    let directory_start = u32_at(&end, ZIP_DIRECTORY_START_OFFSET);
    if &end[..ZIP_END_SIGNATURE.len()] != ZIP_END_SIGNATURE
        || end[ZIP_DISK_FIELDS].iter().any(|byte| *byte != 0)
        || usize::from(count) != output.expected.len()
        || u16_at(&end, ZIP_DISK_COUNT_OFFSET) != count
        || u16_at(&end, ZIP_COMMENT_LENGTH_OFFSET) != 0
        || directory_size > MAX_ZIP_DIRECTORY_BYTES
        || u64::from(directory_start) + u64::from(directory_size) != length - ZIP_END_BYTES as u64
    {
        return Err(UpdateError::Archive);
    }
    reader.seek(SeekFrom::Start(0)).map_err(UpdateError::Io)?;
    let mut archive = ZipArchive::new(reader).map_err(|_| UpdateError::Archive)?;
    if archive.len() != output.expected.len() {
        return Err(UpdateError::Archive);
    }
    for index in 0..archive.len() {
        check_cancelled(output.cancelled)?;
        let mut entry = archive.by_index(index).map_err(|_| UpdateError::Archive)?;
        let mode = entry.unix_mode().ok_or(UpdateError::Archive)?;
        if entry.is_dir() || entry.encrypted() || mode & FILE_TYPE_MASK != REGULAR_FILE_TYPE {
            return Err(UpdateError::Archive);
        }
        let name = output
            .expected
            .iter()
            .copied()
            .find(|name| *name == entry.name())
            .ok_or(UpdateError::Archive)?;
        let size = entry.size();
        output.file(name, size, mode & PERMISSION_MASK, &mut entry)?;
    }
    Ok(())
}

fn u16_at(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

fn extract_tar(reader: File, output: &mut Contents<'_>) -> Result<(), UpdateError> {
    let decoder = GzDecoder::new(BufReader::new(reader));
    let mut archive = tar::Archive::new(decoder.take(MAX_OUTPUT_BYTES + 1));
    for entry in archive
        .entries()
        .map_err(|_| UpdateError::Archive)?
        .raw(true)
    {
        check_cancelled(output.cancelled)?;
        let mut entry = entry.map_err(|_| UpdateError::Archive)?;
        if !entry.header().entry_type().is_file() {
            return Err(UpdateError::Archive);
        }
        let bytes = entry.path_bytes();
        let name = str::from_utf8(&bytes).map_err(|_| UpdateError::Archive)?;
        let name = output
            .expected
            .iter()
            .copied()
            .find(|expected| *expected == name)
            .ok_or(UpdateError::Archive)?;
        let mode = entry.header().mode().map_err(|_| UpdateError::Archive)?;
        let size = entry.size();
        output.file(name, size, mode, &mut entry)?;
    }
    // Finish the gzip checksum and permit only normal TAR zero padding. A second
    // gzip member or any trailing compressed material is not this release format.
    let mut decoder = archive.into_inner();
    let mut buffer = [0; COPY_BYTES];
    loop {
        check_cancelled(output.cancelled)?;
        let count = decoder
            .read(&mut buffer)
            .map_err(|_| UpdateError::Archive)?;
        if count == 0 {
            break;
        }
        if buffer[..count].iter().any(|byte| *byte != 0) {
            return Err(UpdateError::Archive);
        }
    }
    if decoder.limit() == 0 {
        return Err(UpdateError::Archive);
    }
    let mut compressed = decoder.into_inner().into_inner();
    if compressed
        .read(&mut [0])
        .map_err(|_| UpdateError::Archive)?
        != 0
    {
        return Err(UpdateError::Archive);
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    product: String,
    version: String,
    target: String,
    #[serde(deserialize_with = "unique_hashes")]
    files: BTreeMap<String, String>,
}

fn unique_hashes<'de, D>(deserializer: D) -> Result<BTreeMap<String, String>, D::Error>
where
    D: Deserializer<'de>,
{
    struct Hashes;
    impl<'de> Visitor<'de> for Hashes {
        type Value = BTreeMap<String, String>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("unique archive payload hashes")
        }

        fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
        where
            M: MapAccess<'de>,
        {
            let mut files = BTreeMap::new();
            while let Some((name, hash)) = map.next_entry::<String, String>()? {
                if files.insert(name, hash).is_some() {
                    return Err(de::Error::custom("duplicate archive payload hash"));
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

fn canonical_plist(version: &Version) -> String {
    let version = version.to_string();
    let numeric = version
        .split_once('-')
        .map_or(version.as_str(), |(numeric, _)| numeric);
    format!(
        concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
            "<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n",
            "<plist version=\"1.0\">\n<dict>\n",
            "\t<key>CFBundleDisplayName</key>\n\t<string>TableCraft</string>\n",
            "\t<key>CFBundleExecutable</key>\n\t<string>tablecraft-desktop</string>\n",
            "\t<key>CFBundleIconFile</key>\n\t<string>TableCraft.icns</string>\n",
            "\t<key>CFBundleIdentifier</key>\n\t<string>org.replicraft.tablecraft</string>\n",
            "\t<key>CFBundleName</key>\n\t<string>TableCraft</string>\n",
            "\t<key>CFBundlePackageType</key>\n\t<string>APPL</string>\n",
            "\t<key>CFBundleShortVersionString</key>\n\t<string>{numeric}</string>\n",
            "\t<key>CFBundleVersion</key>\n\t<string>{numeric}</string>\n",
            "\t<key>NSHighResolutionCapable</key>\n\t<true/>\n</dict>\n</plist>\n",
        ),
        numeric = numeric
    )
}

#[cfg(test)]
mod tests;

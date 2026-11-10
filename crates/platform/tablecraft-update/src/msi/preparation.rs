use super::{MsiInstallation, handoff, installed::MAX_PAYLOAD_BYTES, native, report::Report};
use crate::{
    UpdateError, VerifiedDownload,
    apply::{AppliedUpdate, ApplyError, ApplyStage, RetainedRecovery},
    artifact::ArtifactKind,
    package,
};
use semver::Version;
use serde_json::json;
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{Read, Write},
    path::Path,
};
use tempfile::TempDir;

const MAX_MANIFEST_BYTES: u64 = 64 * 1024;
const MAX_PAYLOAD_ENTRIES: usize = 32;
const COPY_BUFFER_BYTES: usize = 64 * 1024;

/// An authenticated MSI and native registration proof retained until handoff.
#[derive(Debug)]
pub(crate) struct PreparedMsi {
    directory: TempDir,
    installed: MsiInstallation,
    version: Version,
    product: String,
    digest: [u8; 32],
    executable_digest: String,
    manifest_digest: String,
}

impl PreparedMsi {
    pub(crate) fn prepare(
        download: VerifiedDownload,
        installed: MsiInstallation,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, UpdateError> {
        if download.kind() != ArtifactKind::WindowsMsi
            || download.platform() != installed.platform()
            || download.version() <= installed.version()
            || !download.version().pre.is_empty()
        {
            return Err(UpdateError::ReleaseMetadata);
        }
        installed.revalidate(cancelled)?;
        let mut verified = download.open_verified(cancelled)?;
        let directory = tempfile::Builder::new()
            .prefix(".tablecraft-msi-update-")
            .tempdir_in(installed.staging_parent())
            .map_err(UpdateError::Io)?;
        let msi = directory.path().join("update.msi");
        let mut file = File::create_new(&msi).map_err(UpdateError::Io)?;
        let mut buffer = [0; COPY_BUFFER_BYTES];
        let mut received = 0;
        loop {
            if cancelled() {
                return Err(UpdateError::Cancelled);
            }
            let count = verified.read(&mut buffer).map_err(UpdateError::Io)?;
            if count == 0 {
                break;
            }
            received += count as u64;
            if received > MAX_PAYLOAD_BYTES {
                return Err(UpdateError::AssetTooLarge);
            }
            file.write_all(&buffer[..count]).map_err(UpdateError::Io)?;
        }
        file.sync_all().map_err(UpdateError::Io)?;
        drop(file);
        let digest =
            package::hash_file(&msi, MAX_PAYLOAD_BYTES, cancelled)?.ok_or(UpdateError::Archive)?;
        if &digest != download.digest() {
            return Err(UpdateError::PayloadChanged);
        }
        // Reauthentication reads the download's stable handle again. Extraction
        // receives only the completed private copy, never a server-controlled path.
        drop(download.open_verified(cancelled)?);
        let report: Report = serde_json::from_value(native::inspect(
            &json!({"operation":"inspect", "path":msi}),
            cancelled,
        )?)
        .map_err(|_| UpdateError::Archive)?;
        let product = report
            .validate(download.version(), installed.platform())?
            .to_string();
        if product.eq_ignore_ascii_case(installed.product()) {
            return Err(UpdateError::Archive);
        }
        let cabinet = directory.path().join("payload.cab");
        native::inspect(
            &json!({"operation":"cabinet", "path":msi, "destination":cabinet}),
            cancelled,
        )?;
        let image = directory.path().join("payload");
        super::cabinet::extract(&cabinet, &image, &report, cancelled)?;
        let (executable_digest, manifest_digest) = validate_payload(
            &image,
            &report,
            download.version(),
            installed.platform(),
            cancelled,
        )?;
        if package::hash_file(&msi, MAX_PAYLOAD_BYTES, cancelled)? != Some(digest) {
            return Err(UpdateError::PayloadChanged);
        }
        installed.revalidate(cancelled)?;
        Ok(Self {
            directory,
            installed,
            version: download.into_version(),
            product,
            digest,
            executable_digest,
            manifest_digest,
        })
    }

    pub(crate) fn version(&self) -> &Version {
        &self.version
    }

    pub(crate) fn apply(self, cancelled: &dyn Fn() -> bool) -> Result<AppliedUpdate, ApplyError> {
        self.installed
            .revalidate(cancelled)
            .map_err(ApplyError::Installation)?;
        if package::hash_file(
            &self.directory.path().join("update.msi"),
            MAX_PAYLOAD_BYTES,
            cancelled,
        )
        .map_err(ApplyError::Installation)?
            != Some(self.digest)
        {
            return Err(ApplyError::Installation(UpdateError::PayloadChanged));
        }
        let parent = self
            .installed
            .executable()
            .parent()
            .ok_or(ApplyError::Layout)?
            .to_path_buf();
        let request = json!({
            "parent":std::process::id(), "stage":self.directory.path(),
            "executable":self.installed.executable(), "old_product":self.installed.product(),
            "new_product":self.product, "version":self.version.to_string(),
            "old_executable_hash":package::hash_text(self.installed.executable_digest()),
            "old_manifest_hash":package::hash_text(self.installed.manifest_digest()),
            "msi_hash":package::hash_text(&self.digest),
            "executable_hash":self.executable_digest, "manifest_hash":self.manifest_digest,
        });
        let mut helper = handoff::spawn(&request).map_err(ApplyError::Installation)?;
        let result = (|| {
            handoff::wait_ready(
                &mut helper,
                self.directory.path(),
                &package::hash_text(&self.digest),
                cancelled,
            )?;
            let mut commit =
                File::create_new(self.directory.path().join("COMMIT")).map_err(UpdateError::Io)?;
            commit
                .write_all(package::hash_text(&self.digest).as_bytes())
                .map_err(UpdateError::Io)?;
            commit.sync_all().map_err(UpdateError::Io)
        })();
        let retained = self.directory.keep();
        if let Err(error) = result {
            // A native elevation request may still own the stage. Signal an
            // explicit abort and retain every byte/log instead of dropping it.
            let abort = File::create_new(retained.join("ABORT"));
            if let Ok(file) = abort {
                let _ = file.sync_all();
            }
            return Err(ApplyError::Filesystem {
                source: std::io::Error::other(error),
                recovery: Some(RetainedRecovery::new(retained, parent)),
            });
        }
        Ok(AppliedUpdate::handoff(
            retained,
            parent,
            Some(helper),
            ApplyStage::InstallerScheduled,
        ))
    }
}

pub(super) fn validate_payload(
    root: &Path,
    report: &Report,
    version: &Version,
    platform: crate::platform::Platform,
    cancelled: &dyn Fn() -> bool,
) -> Result<(String, String), UpdateError> {
    let mut pending = vec![root.to_path_buf()];
    let mut files = BTreeMap::new();
    let mut entries = 0;
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory).map_err(UpdateError::Io)? {
            if cancelled() {
                return Err(UpdateError::Cancelled);
            }
            entries += 1;
            if entries > MAX_PAYLOAD_ENTRIES {
                return Err(UpdateError::Archive);
            }
            let entry = entry.map_err(UpdateError::Io)?;
            let kind = entry.file_type().map_err(UpdateError::Io)?;
            if kind.is_dir() {
                pending.push(entry.path());
            } else if kind.is_file() {
                let name = entry
                    .file_name()
                    .into_string()
                    .map_err(|_| UpdateError::Archive)?;
                if ![
                    "tablecraft-desktop.exe",
                    "LICENSE",
                    "ThirdPartyLicenses.txt",
                    "RELEASE.json",
                ]
                .contains(&name.as_str())
                    || files.insert(name, entry.path()).is_some()
                {
                    return Err(UpdateError::Archive);
                }
            } else {
                return Err(UpdateError::Archive);
            }
        }
    }
    if files.len() != 4 {
        return Err(UpdateError::Archive);
    }
    for file in &report.files {
        let name = file
            .name
            .split('|')
            .next_back()
            .ok_or(UpdateError::Archive)?;
        if fs::metadata(&files[name])
            .map_err(UpdateError::Io)?
            .len()
            .to_string()
            != file.size
        {
            return Err(UpdateError::Archive);
        }
    }
    let manifest = read_bounded(&files["RELEASE.json"], MAX_MANIFEST_BYTES)?;
    let metadata: package::Manifest =
        serde_json::from_slice(&manifest).map_err(|_| UpdateError::Archive)?;
    if metadata.installation.is_none() || !metadata.matches_installation(platform, version) {
        return Err(UpdateError::Archive);
    }
    for (name, expected) in &metadata.files {
        let actual = package::hash_file(&files[name], MAX_PAYLOAD_BYTES, cancelled)?
            .ok_or(UpdateError::Archive)?;
        if !package::hash_matches(expected, &actual) {
            return Err(UpdateError::PayloadChanged);
        }
    }
    let executable = &metadata.files["tablecraft-desktop.exe"];
    let manifest_hash = package::hash_file(&files["RELEASE.json"], MAX_MANIFEST_BYTES, cancelled)?
        .ok_or(UpdateError::Archive)?;
    Ok((executable.to_string(), package::hash_text(&manifest_hash)))
}

pub(super) fn read_bounded(path: &Path, maximum: u64) -> Result<Vec<u8>, UpdateError> {
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(UpdateError::Io)?
        .take(maximum + 1)
        .read_to_end(&mut bytes)
        .map_err(UpdateError::Io)?;
    if bytes.len() as u64 > maximum {
        return Err(UpdateError::Archive);
    }
    Ok(bytes)
}

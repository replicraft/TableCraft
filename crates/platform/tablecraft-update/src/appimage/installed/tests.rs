use super::{InstalledAppImage, authenticate_at};
use crate::{
    UpdateError,
    appimage::context::Context,
    archive, artifact,
    http::{Body, Transport},
    platform::Platform,
    trust,
};
use minisign_verify::PublicKey;
use semver::Version;
use std::{fs, io::Cursor, path::PathBuf};
use tempfile::TempDir;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

const ORIGINAL: &[u8] = include_bytes!("../tests/original.bin");
const ORIGINAL_SIGNATURE: &[u8] = include_bytes!("../tests/original.bin.minisig");
const UPDATE: &[u8] = include_bytes!("../tests/update.bin");
const UPDATE_SIGNATURE: &[u8] = include_bytes!("../tests/update.bin.minisig");
const KEY: &str = include_str!("../tests/TEST-ONLY.pub");

struct Source;

impl Transport for Source {
    fn get(&self, url: &str) -> Result<Body, UpdateError> {
        let old = artifact::download_url(
            &Version::new(0, 1, 0),
            "tablecraft-desktop-0.1.0-linux-x64.AppImage.minisig",
        );
        let new = artifact::download_url(
            &Version::new(0, 1, 1),
            "tablecraft-desktop-0.1.1-linux-x64.AppImage",
        );
        let bytes = if url == old {
            ORIGINAL_SIGNATURE
        } else if url == format!("{new}.minisig") {
            UPDATE_SIGNATURE
        } else if url == new {
            UPDATE
        } else {
            return Err(UpdateError::UnauthorizedUrl);
        };
        Ok(Body {
            reader: Box::new(Cursor::new(bytes)),
            length: Some(bytes.len() as u64),
        })
    }
}

struct Fixture {
    _root: TempDir,
    appdir: PathBuf,
    image: PathBuf,
    executable: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let appdir = root.path().join(".mount_TableCraft");
        let bin = appdir.join("usr/bin");
        fs::create_dir_all(&bin).unwrap();
        for (name, bytes) in archive::tests::files(Platform::LinuxX64, "0.1.0") {
            fs::write(bin.join(name), bytes).unwrap();
        }
        let image = root.path().join("My renamed application.AppImage");
        fs::write(&image, ORIGINAL).unwrap();
        Self {
            _root: root,
            appdir,
            image,
            executable: bin.join("tablecraft-desktop"),
        }
    }

    fn authenticate(&self) -> Result<InstalledAppImage, UpdateError> {
        self.authenticate_with(&trust::parse_key(KEY).unwrap(), &|| false)
    }

    fn authenticate_with(
        &self,
        key: &PublicKey,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<InstalledAppImage, UpdateError> {
        authenticate_at(
            &Source,
            key,
            Context {
                image: self.image.clone(),
                appdir: self.appdir.clone(),
            },
            &self.executable,
            &Version::new(0, 1, 0),
            Platform::LinuxX64,
            cancelled,
        )
    }
}

#[test]
fn original_image_requires_publisher_signature_runtime_identity_and_fresh_bytes() {
    let fixture = Fixture::new();
    let installed = fixture.authenticate().unwrap();
    assert_eq!(installed.image(), fixture.image.canonicalize().unwrap());
    assert_eq!(
        installed.staging_parent(),
        fixture.image.parent().unwrap().canonicalize().unwrap()
    );
    let mut altered = ORIGINAL.to_vec();
    *altered.last_mut().unwrap() ^= 1;
    fs::write(&fixture.image, &altered).unwrap();
    assert!(matches!(
        installed.revalidate(&|| false),
        Err(UpdateError::PayloadChanged)
    ));
    assert!(matches!(
        fixture.authenticate(),
        Err(UpdateError::Signature)
    ));
    fs::write(&fixture.image, ORIGINAL).unwrap();
    fs::write(&fixture.executable, b"changed runtime executable").unwrap();
    assert!(matches!(
        fixture.authenticate(),
        Err(UpdateError::UnqualifiedInstallation)
    ));
}

#[test]
fn environment_cannot_authorize_unrelated_images_or_wrong_publishers() {
    let fixture = Fixture::new();
    let other_key = trust::parse_key(include_str!("../../download/tests/TEST-ONLY.pub")).unwrap();
    assert!(matches!(
        fixture.authenticate_with(&other_key, &|| false),
        Err(UpdateError::Signature)
    ));
    let inner = fixture.appdir.join("nested.AppImage");
    fs::write(&inner, ORIGINAL).unwrap();
    assert!(matches!(
        authenticate_at(
            &Source,
            &trust::parse_key(KEY).unwrap(),
            Context {
                image: inner,
                appdir: fixture.appdir.clone()
            },
            &fixture.executable,
            &Version::new(0, 1, 0),
            Platform::LinuxX64,
            &|| false
        ),
        Err(UpdateError::UnqualifiedInstallation)
    ));
    assert!(matches!(
        fixture.authenticate_with(&trust::parse_key(KEY).unwrap(), &|| true),
        Err(UpdateError::Cancelled)
    ));
}

#[test]
fn original_signature_is_bound_to_exact_version_and_native_architecture() {
    let fixture = Fixture::new();
    let key = trust::parse_key(KEY).unwrap();
    let signature = crate::verification::signature(
        ORIGINAL_SIGNATURE,
        &Version::new(0, 1, 1),
        "tablecraft-desktop-0.1.0-linux-x64.AppImage",
    );
    assert!(matches!(signature, Err(UpdateError::Signature)));
    let mut file = fs::File::open(&fixture.image).unwrap();
    assert!(matches!(
        crate::appimage::payload::check(&mut file, Platform::LinuxArm64),
        Err(UpdateError::Archive)
    ));
    let wrong_executable = fixture.appdir.join("usr/bin/other");
    fs::write(&wrong_executable, b"not the running product").unwrap();
    assert!(matches!(
        authenticate_at(
            &Source,
            &key,
            Context {
                image: fixture.image.clone(),
                appdir: fixture.appdir.clone()
            },
            &wrong_executable,
            &Version::new(0, 1, 0),
            Platform::LinuxX64,
            &|| false
        ),
        Err(UpdateError::UnqualifiedInstallation)
    ));
}

#[cfg(unix)]
#[test]
fn independently_signed_new_image_prepares_without_running_or_changing_original() {
    let fixture = Fixture::new();
    fs::set_permissions(&fixture.image, fs::Permissions::from_mode(0o4755)).unwrap();
    let target = fixture.authenticate().unwrap();
    let name = "tablecraft-desktop-0.1.1-linux-x64.AppImage";
    let url = artifact::download_url(&Version::new(0, 1, 1), name);
    let metadata = serde_json::json!({
        "tag_name": "v0.1.1", "draft": false, "prerelease": false,
        "html_url": "https://github.com/replicraft/TableCraft/releases/tag/v0.1.1",
        "assets": [
            { "name": name, "size": UPDATE.len(), "browser_download_url": url },
            { "name": format!("{name}.minisig"), "size": UPDATE_SIGNATURE.len(), "browser_download_url": format!("{url}.minisig") }
        ]
    });
    let release = crate::release::select(
        &serde_json::to_vec(&metadata).unwrap(),
        &Version::new(0, 1, 0),
        Platform::LinuxX64,
        crate::artifact::ArtifactKind::AppImage,
    )
    .unwrap()
    .unwrap();
    let download = crate::download::download(
        &Source,
        &trust::parse_key(KEY).unwrap(),
        &release,
        target.staging_parent(),
        &mut |_| true,
    )
    .unwrap();
    let staged = download.staged_path().to_path_buf();
    let prepared =
        crate::appimage::preparation::PreparedAppImage::prepare(download, target, &|| false)
            .unwrap();
    assert_eq!(prepared.version(), &Version::new(0, 1, 1));
    assert_eq!(
        fs::metadata(staged).unwrap().permissions().mode() & 0o7777,
        0o755
    );
    assert_eq!(fs::read(&fixture.image).unwrap(), ORIGINAL);
    assert!(matches!(
        prepared.apply(&|| true),
        Err(crate::apply::ApplyError::Installation(
            UpdateError::Cancelled
        ))
    ));
    assert_eq!(fs::read(&fixture.image).unwrap(), ORIGINAL);
}

#[test]
fn inert_header_admission_refuses_non_elf_wrong_word_order_tag_and_machine() {
    let fixture = Fixture::new();
    for offset in [0, 4, 5, 8, 16, 18] {
        let mut changed = ORIGINAL.to_vec();
        changed[offset] ^= 0x40;
        fs::write(&fixture.image, changed).unwrap();
        let mut file = fs::File::open(&fixture.image).unwrap();
        assert!(
            matches!(
                crate::appimage::payload::check(&mut file, Platform::LinuxX64),
                Err(UpdateError::Archive)
            ),
            "header offset {offset}"
        );
    }
}

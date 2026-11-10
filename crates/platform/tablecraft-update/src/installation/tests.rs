use super::{InstallPlan, Installed, ManualReason, classify, managed_location};
use crate::{UpdateError, platform::Platform};
use semver::Version;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};
use tempfile::TempDir;

const RUNNING: &str = "0.1.0";
const EXECUTABLE: &[u8] = b"independent local desktop executable payload";

struct Fixture {
    _directory: TempDir,
    executable: PathBuf,
    manifest: PathBuf,
    metadata: Value,
    platform: Platform,
}

impl Fixture {
    fn portable(platform: Platform) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let filename = if matches!(platform, Platform::WindowsX64 | Platform::WindowsArm64) {
            "tablecraft-desktop.exe"
        } else {
            "tablecraft-desktop"
        };
        let executable = directory.path().join(filename);
        fs::write(&executable, EXECUTABLE).unwrap();
        fs::write(directory.path().join("LICENSE"), b"project license").unwrap();
        fs::write(
            directory.path().join("ThirdPartyLicenses.txt"),
            b"dependency notices",
        )
        .unwrap();
        let manifest = directory.path().join("RELEASE.json");
        let metadata = json!({
            "product": "tablecraft-desktop",
            "version": RUNNING,
            "target": platform.target(),
            "files": {
                filename: format!("{:x}", Sha256::digest(EXECUTABLE)),
                "LICENSE": format!("{:x}", Sha256::digest(b"project license")),
                "ThirdPartyLicenses.txt": format!("{:x}", Sha256::digest(b"dependency notices")),
            },
        });
        let fixture = Self {
            _directory: directory,
            executable,
            manifest,
            metadata,
            platform,
        };
        fixture.write_manifest();
        fixture
    }

    fn mac() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let contents = directory.path().join("TableCraft.app/Contents");
        fs::create_dir_all(contents.join("MacOS")).unwrap();
        fs::create_dir_all(contents.join("Resources")).unwrap();
        let executable = contents.join("MacOS/tablecraft-desktop");
        fs::write(&executable, EXECUTABLE).unwrap();
        fs::write(contents.join("Info.plist"), b"independent bundle plist").unwrap();
        for name in ["LICENSE", "ThirdPartyLicenses.txt", "TableCraft.icns"] {
            fs::write(
                contents.join("Resources").join(name),
                b"independent resource",
            )
            .unwrap();
        }
        let manifest = contents.join("Resources/RELEASE.json");
        let metadata = json!({
            "product": "tablecraft-desktop",
            "version": RUNNING,
            "target": "aarch64-apple-darwin",
            "files": {
                "TableCraft.app/Contents/MacOS/tablecraft-desktop": format!("{:x}", Sha256::digest(EXECUTABLE)),
                "TableCraft.app/Contents/Info.plist": format!("{:x}", Sha256::digest(b"independent bundle plist")),
                "TableCraft.app/Contents/Resources/LICENSE": format!("{:x}", Sha256::digest(b"independent resource")),
                "TableCraft.app/Contents/Resources/ThirdPartyLicenses.txt": format!("{:x}", Sha256::digest(b"independent resource")),
                "TableCraft.app/Contents/Resources/TableCraft.icns": format!("{:x}", Sha256::digest(b"independent resource")),
            },
        });
        let fixture = Self {
            _directory: directory,
            executable,
            manifest,
            metadata,
            platform: Platform::MacosArm64,
        };
        fixture.write_manifest();
        fixture
    }

    fn write_manifest(&self) {
        fs::write(&self.manifest, serde_json::to_vec(&self.metadata).unwrap()).unwrap();
    }

    fn plan(&self) -> InstallPlan {
        classify(
            &self.executable,
            self.platform,
            &Version::parse(RUNNING).unwrap(),
            false,
        )
        .unwrap()
    }
}

fn installed(plan: InstallPlan) -> Installed {
    match plan {
        InstallPlan::Portable(installed) | InstallPlan::MacBundle(installed) => installed,
        InstallPlan::Manual(reason) => panic!("expected installed package, got {reason:?}"),
    }
}

fn manual(plan: InstallPlan, expected: ManualReason) {
    assert!(matches!(plan, InstallPlan::Manual(reason) if reason == expected));
}

#[test]
fn detects_manifest_owned_flat_packages_and_rechecks_without_mutation() {
    for platform in [
        Platform::WindowsX64,
        Platform::WindowsArm64,
        Platform::LinuxX64,
        Platform::LinuxArm64,
    ] {
        let fixture = Fixture::portable(platform);
        let installed = installed(fixture.plan());
        assert_eq!(
            installed.executable(),
            fixture.executable.canonicalize().unwrap()
        );
        assert_eq!(
            installed.directory(),
            installed.executable().parent().unwrap()
        );
        assert_eq!(
            installed.manifest(),
            installed.directory().join("RELEASE.json")
        );
        assert_eq!(installed.bundle(), None);
        assert_eq!(installed.platform(), platform);
        assert_eq!(installed.version(), &Version::parse(RUNNING).unwrap());
        installed.revalidate().unwrap();
        assert_eq!(fs::read(installed.executable()).unwrap(), EXECUTABLE);
    }
}

#[test]
fn dragged_mac_bundle_keeps_its_own_manifest_and_staging_parent() {
    let mut fixture = Fixture::mac();
    let destination = tempfile::tempdir().unwrap();
    fs::rename(
        fixture._directory.path().join("TableCraft.app"),
        destination.path().join("Renamed.app"),
    )
    .unwrap();
    fixture.executable = destination
        .path()
        .join("Renamed.app/Contents/MacOS/tablecraft-desktop");
    fixture.manifest = destination
        .path()
        .join("Renamed.app/Contents/Resources/RELEASE.json");

    let plan = fixture.plan();
    assert!(matches!(plan, InstallPlan::MacBundle(_)));
    let installed = installed(plan);
    assert_eq!(
        installed.directory(),
        destination.path().canonicalize().unwrap()
    );
    assert_eq!(
        installed.bundle().unwrap(),
        installed.directory().join("Renamed.app")
    );
    assert!(
        installed
            .manifest()
            .ends_with("Contents/Resources/RELEASE.json")
    );
    installed.revalidate().unwrap();
}

#[test]
fn debug_and_cargo_output_trees_are_manual_even_with_valid_manifests() {
    let fixture = Fixture::portable(Platform::LinuxX64);
    manual(
        classify(
            &fixture.executable,
            fixture.platform,
            &Version::parse(RUNNING).unwrap(),
            true,
        )
        .unwrap(),
        ManualReason::DevelopmentBuild,
    );
    fs::write(
        fixture._directory.path().join("CACHEDIR.TAG"),
        b"Signature: 8a477f597d28d172789f06886806bc55\n",
    )
    .unwrap();
    manual(fixture.plan(), ManualReason::DevelopmentBuild);
}

#[test]
fn missing_manifest_and_external_mac_archive_manifest_do_not_grant_ownership() {
    let fixture = Fixture::portable(Platform::LinuxX64);
    fs::remove_file(&fixture.manifest).unwrap();
    manual(fixture.plan(), ManualReason::MissingManifest);

    let fixture = Fixture::mac();
    fs::rename(
        &fixture.manifest,
        fixture._directory.path().join("RELEASE.json"),
    )
    .unwrap();
    manual(fixture.plan(), ManualReason::MissingManifest);
}

#[test]
fn wrong_product_target_version_hash_and_file_membership_are_manual() {
    for (key, value) in [
        ("product", json!("other-app")),
        ("target", json!("aarch64-apple-darwin")),
        ("version", json!("0.2.0")),
    ] {
        let mut fixture = Fixture::portable(Platform::LinuxX64);
        fixture.metadata[key] = value;
        fixture.write_manifest();
        manual(fixture.plan(), ManualReason::InvalidManifest);
    }
    for value in [
        json!("0".repeat(64)),
        json!("f".repeat(63)),
        json!("A".repeat(64)),
    ] {
        let mut fixture = Fixture::portable(Platform::LinuxX64);
        fixture.metadata["files"]["tablecraft-desktop"] = value;
        fixture.write_manifest();
        manual(fixture.plan(), ManualReason::InvalidManifest);
    }
    let mut fixture = Fixture::portable(Platform::LinuxX64);
    fixture.metadata["files"]["../../unrelated"] = json!("0".repeat(64));
    fixture.write_manifest();
    manual(fixture.plan(), ManualReason::InvalidManifest);
}

#[test]
fn changed_executable_or_manifest_invalidates_a_previously_detected_package() {
    let fixture = Fixture::portable(Platform::LinuxX64);
    let installed = installed(fixture.plan());
    fs::write(&fixture.executable, b"changed after detection").unwrap();
    assert!(matches!(
        installed.revalidate(),
        Err(UpdateError::PayloadChanged)
    ));

    let fixture = Fixture::portable(Platform::LinuxX64);
    let installed = installed(fixture.plan());
    let mut bytes = fs::read(&fixture.manifest).unwrap();
    bytes.push(b'\n');
    fs::write(&fixture.manifest, bytes).unwrap();
    assert!(matches!(
        installed.revalidate(),
        Err(UpdateError::PayloadChanged)
    ));
}

#[test]
fn oversized_and_duplicate_field_metadata_are_manual() {
    let fixture = Fixture::portable(Platform::LinuxX64);
    fs::write(&fixture.manifest, vec![b' '; 64 * 1024 + 1]).unwrap();
    manual(fixture.plan(), ManualReason::InvalidManifest);

    let fixture = Fixture::portable(Platform::LinuxX64);
    let mut text = serde_json::to_string(&fixture.metadata).unwrap();
    text = text.replacen("\"product\":", "\"product\":\"duplicate\",\"product\":", 1);
    fs::write(&fixture.manifest, text).unwrap();
    manual(fixture.plan(), ManualReason::InvalidManifest);

    let fixture = Fixture::portable(Platform::LinuxX64);
    let text = serde_json::to_string(&fixture.metadata).unwrap().replacen(
        "\"LICENSE\":",
        &format!("\"LICENSE\":\"{}\",\"LICENSE\":", "0".repeat(64)),
        1,
    );
    fs::write(&fixture.manifest, text).unwrap();
    manual(fixture.plan(), ManualReason::InvalidManifest);
}

#[test]
fn managed_system_locations_and_unrecognized_executables_are_manual() {
    for location in [
        "/usr/bin/tablecraft-desktop",
        "/bin/tablecraft-desktop",
        "/snap/app/tablecraft-desktop",
        "/var/lib/flatpak/app/tablecraft-desktop",
    ] {
        assert!(managed_location(Path::new(location), Platform::LinuxX64));
    }
    assert!(!managed_location(
        Path::new("/opt/user-portable/tablecraft-desktop"),
        Platform::LinuxX64
    ));
    assert!(managed_location(
        Path::new("/Program Files/TableCraft/tablecraft-desktop.exe"),
        Platform::WindowsX64
    ));

    let fixture = Fixture::portable(Platform::LinuxX64);
    let wrong = fixture._directory.path().join("TableCraft.AppImage");
    fs::rename(&fixture.executable, &wrong).unwrap();
    manual(
        classify(
            &wrong,
            fixture.platform,
            &Version::parse(RUNNING).unwrap(),
            false,
        )
        .unwrap(),
        ManualReason::UnrecognizedLayout,
    );
}

#[cfg(unix)]
#[test]
fn symlinked_executables_and_manifests_are_not_package_owned_files() {
    use std::os::unix::fs::symlink;

    let fixture = Fixture::portable(Platform::LinuxX64);
    let real = fixture._directory.path().join("other");
    fs::rename(&fixture.executable, &real).unwrap();
    symlink(&real, &fixture.executable).unwrap();
    manual(fixture.plan(), ManualReason::UnrecognizedLayout);

    let fixture = Fixture::portable(Platform::LinuxX64);
    let real = fixture._directory.path().join("other-manifest");
    fs::rename(&fixture.manifest, &real).unwrap();
    symlink(&real, &fixture.manifest).unwrap();
    manual(fixture.plan(), ManualReason::InvalidManifest);
}

#[test]
fn missing_declared_resources_and_empty_executables_are_manual() {
    let fixture = Fixture::portable(Platform::LinuxX64);
    fs::remove_file(fixture._directory.path().join("LICENSE")).unwrap();
    manual(fixture.plan(), ManualReason::InvalidManifest);

    let fixture = Fixture::portable(Platform::LinuxX64);
    fs::write(&fixture.executable, b"").unwrap();
    manual(fixture.plan(), ManualReason::InvalidManifest);
}

use super::{
    ApplyError,
    helper::{HelperPlan, unix_command, windows_command},
    transaction::{FileMoves, apply_with},
};
use crate::{
    archive::{self, ExtractedUpdate},
    installation::{self, InstallPlan, Installed},
    platform::Platform,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use semver::Version;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    ffi::OsStr,
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};
use tempfile::{NamedTempFile, TempDir};

struct Fixture {
    _root: TempDir,
    directory: PathBuf,
    original: Vec<(String, Vec<u8>)>,
    replacement: Vec<(String, Vec<u8>)>,
    platform: Platform,
}

impl Fixture {
    fn new(platform: Platform) -> Self {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("installed with spaces ' $() ;");
        fs::create_dir(&directory).unwrap();
        let original = archive::tests::files(platform, "0.1.0");
        for (name, bytes) in &original {
            let file = directory.join(name);
            fs::create_dir_all(file.parent().unwrap()).unwrap();
            fs::write(file, bytes).unwrap();
        }
        fs::write(
            directory.join("important.accdb"),
            b"unchanged private database",
        )
        .unwrap();
        fs::create_dir(directory.join("user directory")).unwrap();
        fs::write(
            directory.join("user directory/private.mdb"),
            b"unchanged second database",
        )
        .unwrap();

        let mut replacement = archive::tests::files(platform, "0.1.1");
        replacement[0].1 = b"new executable bytes".to_vec();
        let manifest = replacement
            .iter()
            .position(|(name, _)| name.ends_with("RELEASE.json"))
            .unwrap();
        let mut metadata: Value = serde_json::from_slice(&replacement[manifest].1).unwrap();
        metadata["files"][&replacement[0].0] = json!(hash(&replacement[0].1));
        replacement[manifest].1 = serde_json::to_vec(&metadata).unwrap();
        Self {
            _root: root,
            directory,
            original,
            replacement,
            platform,
        }
    }

    fn installed(&self) -> Installed {
        let executable = self.directory.join(&self.original[0].0);
        match installation::classify(&executable, self.platform, &Version::new(0, 1, 0), false)
            .unwrap()
        {
            InstallPlan::Portable(installed) | InstallPlan::MacBundle(installed) => installed,
            InstallPlan::Manual(reason) => panic!("fixture source did not qualify: {reason:?}"),
        }
    }

    fn extracted(&self) -> ExtractedUpdate {
        let bytes = if matches!(self.platform, Platform::LinuxX64 | Platform::LinuxArm64) {
            archive::tests::tar(&self.replacement, false)
        } else {
            archive::tests::zip(&self.replacement)
        };
        let mut file = NamedTempFile::new_in(self._root.path()).unwrap();
        file.write_all(&bytes).unwrap();
        archive::extract(
            file.reopen().unwrap(),
            self.platform,
            &Version::new(0, 1, 1),
            &self.directory,
            &|| false,
        )
        .unwrap()
    }

    fn assert_source(&self) {
        for (name, bytes) in &self.original {
            assert_eq!(
                &fs::read(self.directory.join(name)).unwrap(),
                bytes,
                "{name}"
            );
        }
        self.assert_databases();
    }

    fn assert_replacement(&self) {
        for (name, bytes) in &self.replacement {
            assert_eq!(
                &fs::read(self.directory.join(name)).unwrap(),
                bytes,
                "{name}"
            );
        }
        self.assert_databases();
    }

    fn assert_databases(&self) {
        assert_eq!(
            fs::read(self.directory.join("important.accdb")).unwrap(),
            b"unchanged private database"
        );
        assert_eq!(
            fs::read(self.directory.join("user directory/private.mdb")).unwrap(),
            b"unchanged second database"
        );
    }

    fn no_backup(&self) {
        assert!(fs::read_dir(&self.directory).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".tablecraft-backup-")
        }));
    }
}

fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

struct Moves {
    calls: usize,
    failures: BTreeSet<usize>,
}

impl Moves {
    fn succeed() -> Self {
        Self {
            calls: 0,
            failures: BTreeSet::new(),
        }
    }

    fn failing(calls: impl IntoIterator<Item = usize>) -> Self {
        Self {
            calls: 0,
            failures: calls.into_iter().collect(),
        }
    }
}

impl FileMoves for Moves {
    fn rename(&mut self, source: &Path, destination: &Path) -> io::Result<()> {
        self.calls += 1;
        if self.failures.contains(&self.calls) {
            return Err(io::Error::other("injected replacement failure"));
        }
        fs::rename(source, destination)
    }
}

#[test]
fn flat_replacement_retains_complete_originals_and_preserves_user_files() {
    for platform in [Platform::WindowsX64, Platform::LinuxX64] {
        let fixture = Fixture::new(platform);
        let mut started = false;
        let applied = apply_with(
            fixture.installed(),
            fixture.extracted(),
            &mut Moves::succeed(),
            &mut |plan| {
                assert_eq!(
                    plan.launch,
                    fixture
                        .directory
                        .canonicalize()
                        .unwrap()
                        .join(&fixture.original[0].0)
                );
                assert!(!plan.bundle);
                assert!(plan.backup.join("READY").is_file());
                started = true;
                Ok(None)
            },
        )
        .unwrap();
        assert!(started);
        fixture.assert_replacement();
        for (name, bytes) in &fixture.original {
            assert_eq!(
                &fs::read(applied.backup_directory().join(name)).unwrap(),
                bytes
            );
        }
        let backup = applied.backup_directory().to_path_buf();
        drop(applied);
        assert!(
            backup.exists(),
            "dropping status must never delete the original backup"
        );
    }
}

#[test]
fn mac_bundle_replacement_retains_original_bundle_and_uses_its_actual_launch_anchor() {
    let fixture = Fixture::new(Platform::MacosArm64);
    let applied = apply_with(
        fixture.installed(),
        fixture.extracted(),
        &mut Moves::succeed(),
        &mut |plan| {
            assert!(plan.bundle);
            assert_eq!(
                plan.launch,
                fixture
                    .directory
                    .canonicalize()
                    .unwrap()
                    .join("TableCraft.app")
            );
            Ok(None)
        },
    )
    .unwrap();
    fixture.assert_replacement();
    for (name, bytes) in &fixture.original {
        let relative = name.strip_prefix("TableCraft.app/").unwrap();
        assert_eq!(
            &fs::read(
                applied
                    .backup_directory()
                    .join("original.app")
                    .join(relative)
            )
            .unwrap(),
            bytes
        );
    }
}

#[test]
fn every_single_move_failure_restores_originals_without_starting_helper() {
    for (platform, move_count) in [
        (Platform::WindowsX64, 8),
        (Platform::LinuxX64, 8),
        (Platform::MacosArm64, 2),
    ] {
        for failure in 1..=move_count {
            let fixture = Fixture::new(platform);
            let mut started = false;
            let result = apply_with(
                fixture.installed(),
                fixture.extracted(),
                &mut Moves::failing([failure]),
                &mut |_| {
                    started = true;
                    Ok(None)
                },
            );
            let error = result.unwrap_err();
            assert!(
                error.retained_recovery().is_none(),
                "single failure should fully roll back: {platform:?}/{failure}"
            );
            assert!(!started);
            fixture.assert_source();
            fixture.no_backup();
        }
    }
}

#[test]
fn helper_spawn_failure_rolls_back_both_native_layouts() {
    for platform in [
        Platform::WindowsX64,
        Platform::LinuxX64,
        Platform::MacosArm64,
    ] {
        let fixture = Fixture::new(platform);
        let error = apply_with(
            fixture.installed(),
            fixture.extracted(),
            &mut Moves::succeed(),
            &mut |_| Err(io::Error::other("injected helper startup failure")),
        )
        .unwrap_err();
        assert!(error.retained_recovery().is_none());
        fixture.assert_source();
        fixture.no_backup();
    }
}

#[test]
fn rollback_failure_retains_every_original_in_installation_or_permanent_backup() {
    for failures in [[2, 3], [4, 5], [4, 6]] {
        let fixture = Fixture::new(Platform::WindowsX64);
        let error = apply_with(
            fixture.installed(),
            fixture.extracted(),
            &mut Moves::failing(failures),
            &mut |_| Ok(None),
        )
        .unwrap_err();
        let recovery = error
            .retained_recovery()
            .expect("failed rollback must retain backup ownership");
        let backup = recovery.backup_directory().to_path_buf();
        assert_eq!(
            recovery.installation_directory(),
            fixture.directory.canonicalize().unwrap()
        );
        drop(error);
        for (name, original) in &fixture.original {
            let installed = fs::read(fixture.directory.join(name)).ok();
            let retained = fs::read(backup.join(name)).ok();
            assert!(
                installed.as_deref() == Some(original) || retained.as_deref() == Some(original),
                "lost original {name} after failures {failures:?}"
            );
        }
        fixture.assert_databases();
        assert!(
            backup.is_dir(),
            "dropping the error must retain recovery files"
        );
    }
}

#[test]
fn stale_installation_or_unknown_bundle_contents_refuse_before_mutation() {
    let fixture = Fixture::new(Platform::WindowsX64);
    let installed = fixture.installed();
    let extracted = fixture.extracted();
    fs::write(installed.executable(), b"changed by another owner").unwrap();
    let result = apply_with(installed, extracted, &mut Moves::succeed(), &mut |_| {
        panic!("helper must not start")
    });
    assert!(matches!(result, Err(ApplyError::Installation(_))));
    fixture.no_backup();
    assert_eq!(
        fs::read(fixture.directory.join(&fixture.original[0].0)).unwrap(),
        b"changed by another owner"
    );

    let fixture = Fixture::new(Platform::MacosArm64);
    let user_file = fixture
        .directory
        .join("TableCraft.app/Contents/private.accdb");
    fs::write(&user_file, b"unowned database inside bundle").unwrap();
    let result = apply_with(
        fixture.installed(),
        fixture.extracted(),
        &mut Moves::succeed(),
        &mut |_| panic!("helper must not start"),
    );
    assert!(matches!(result, Err(ApplyError::Layout)));
    fixture.assert_source();
    fixture.no_backup();
    assert_eq!(
        fs::read(user_file).unwrap(),
        b"unowned database inside bundle"
    );
}

#[test]
fn native_helper_commands_keep_paths_out_of_fixed_script_source() {
    let launch = Path::new("/tmp/a 'quoted' $(bad); name/tablecraft-desktop");
    let working = Path::new("/tmp/working with spaces");
    let backup = Path::new("/tmp/backup ' $() ;");
    let plan = HelperPlan {
        process: 12345,
        launch,
        working_directory: working,
        backup,
        bundle: false,
    };
    let unix = unix_command(&plan);
    assert_eq!(unix.get_program(), OsStr::new("/bin/sh"));
    let arguments: Vec<_> = unix.get_args().collect();
    assert_eq!(arguments[0], "-c");
    assert!(!arguments[1].to_string_lossy().contains("$(bad)"));
    assert_eq!(arguments[4], launch);
    assert_eq!(arguments[5], working);
    assert_eq!(arguments[6], backup);
    assert_eq!(arguments[7], "portable");

    let windows = windows_command(&plan, PathBuf::from("/system/powershell.exe"));
    let arguments: Vec<_> = windows.get_args().collect();
    assert_eq!(arguments[3], "-EncodedCommand");
    let encoded = STANDARD.decode(arguments[4].to_str().unwrap()).unwrap();
    let units: Vec<_> = encoded
        .chunks_exact(2)
        .map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]))
        .collect();
    let script = String::from_utf16(&units).unwrap();
    assert!(!script.contains("$(bad)"));
    let environment: Vec<_> = windows.get_envs().collect();
    assert!(environment.contains(&(
        OsStr::new("TABLECRAFT_UPDATE_LAUNCH"),
        Some(launch.as_os_str())
    )));
    assert!(environment.contains(&(
        OsStr::new("TABLECRAFT_UPDATE_BACKUP"),
        Some(backup.as_os_str())
    )));
}

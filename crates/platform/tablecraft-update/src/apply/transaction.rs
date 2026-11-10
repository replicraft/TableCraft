use super::{
    helper::{self, HelperPlan},
    outcome::{AppliedUpdate, ApplyError, RetainedRecovery},
};
use crate::{archive::ExtractedUpdate, installation::Installed, platform::Platform};
use std::{
    fs::{self, OpenOptions},
    io,
    path::{Path, PathBuf},
    process::Child,
};

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
const READY: &str = "READY";
const MAC_MEMBERS: [&str; 6] = [
    "Contents/MacOS/tablecraft-desktop",
    "Contents/Info.plist",
    "Contents/Resources/LICENSE",
    "Contents/Resources/ThirdPartyLicenses.txt",
    "Contents/Resources/TableCraft.icns",
    "Contents/Resources/RELEASE.json",
];
const MAC_DIRECTORIES: [&str; 3] = ["Contents", "Contents/MacOS", "Contents/Resources"];

pub(crate) fn apply(
    installed: Installed,
    extracted: ExtractedUpdate,
) -> Result<AppliedUpdate, ApplyError> {
    apply_with(installed, extracted, &mut NativeFiles, &mut helper::spawn)
}

pub(super) trait FileMoves {
    fn rename(&mut self, source: &Path, destination: &Path) -> io::Result<()>;
}

struct NativeFiles;

impl FileMoves for NativeFiles {
    fn rename(&mut self, source: &Path, destination: &Path) -> io::Result<()> {
        fs::rename(source, destination)
    }
}

struct Replacement {
    installed: PathBuf,
    staged: PathBuf,
    backup: PathBuf,
    original_moved: bool,
    replacement_moved: bool,
}

pub(super) fn apply_with(
    installed: Installed,
    extracted: ExtractedUpdate,
    files: &mut dyn FileMoves,
    spawn: &mut dyn FnMut(&HelperPlan<'_>) -> io::Result<Option<Child>>,
) -> Result<AppliedUpdate, ApplyError> {
    installed.revalidate().map_err(ApplyError::Installation)?;
    if let Some(bundle) = installed.bundle()
        && !owned_bundle(bundle).map_err(|source| ApplyError::Filesystem {
            source,
            recovery: None,
        })?
    {
        return Err(ApplyError::Layout);
    }
    let backup = tempfile::Builder::new()
        .prefix(".tablecraft-backup-")
        .tempdir_in(installed.directory())
        .map_err(|source| ApplyError::Filesystem {
            source,
            recovery: None,
        })?;
    let mut replacements = replacements(&installed, &extracted, backup.path())?;

    // This is intentionally permanent before any original moves. Neither an
    // error return nor TempDir Drop may delete the only remaining original.
    let backup = backup.keep();
    let result = replace(&mut replacements, files)
        .and_then(|()| {
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(backup.join(READY))?
                .sync_all()
        })
        .and_then(|()| {
            let plan = HelperPlan {
                process: std::process::id(),
                launch: installed.bundle().unwrap_or_else(|| installed.executable()),
                working_directory: installed.directory(),
                backup: &backup,
                bundle: installed.bundle().is_some(),
            };
            spawn(&plan)
        });

    match result {
        Ok(helper) => Ok(AppliedUpdate {
            helper,
            // NO_CLONE: recovery outlives the consumed Installed proof; retain
            // one independent local anchor without copying application payloads.
            recovery: RetainedRecovery {
                backup,
                installation: installed.directory().to_path_buf(),
            },
        }),
        Err(source) => {
            let restored = rollback(&mut replacements, files);
            let recovery = if restored && cleanup_empty_backup(&backup).is_ok() {
                None
            } else {
                // NO_CLONE: a failed rollback must return its own recovery
                // anchor after the source proof and staged update are dropped.
                Some(RetainedRecovery {
                    backup,
                    installation: installed.directory().to_path_buf(),
                })
            };
            Err(ApplyError::Filesystem { source, recovery })
        }
    }
}

fn replacements(
    installed: &Installed,
    extracted: &ExtractedUpdate,
    backup: &Path,
) -> Result<Vec<Replacement>, ApplyError> {
    if let Some(bundle) = installed.bundle() {
        let staged = extracted.bundle().ok_or(ApplyError::Layout)?;
        return Ok(vec![Replacement {
            installed: bundle.to_path_buf(),
            staged: staged.to_path_buf(),
            backup: backup.join("original.app"),
            original_moved: false,
            replacement_moved: false,
        }]);
    }
    if extracted.bundle().is_some() {
        return Err(ApplyError::Layout);
    }
    let members = match installed.platform() {
        Platform::WindowsX64 | Platform::WindowsArm64 => &WINDOWS_FILES,
        Platform::LinuxX64 | Platform::LinuxArm64 => &LINUX_FILES,
        Platform::MacosX64 | Platform::MacosArm64 => return Err(ApplyError::Layout),
    };
    // Four independent move states are retained until replacement or rollback
    // finishes; this is the exact portable package set, never a directory census.
    Ok(members
        .iter()
        .map(|name| Replacement {
            installed: installed.directory().join(name),
            staged: if *name == members[0] {
                extracted.executable().to_path_buf()
            } else {
                extracted.root().join(name)
            },
            backup: backup.join(name),
            original_moved: false,
            replacement_moved: false,
        })
        .collect())
}

fn owned_bundle(bundle: &Path) -> io::Result<bool> {
    let mut pending = vec![bundle.to_path_buf()];
    let mut members = 0;
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let path = entry.path();
            let relative = path
                .strip_prefix(bundle)
                .map_err(|_| io::Error::other("Invalid application bundle member"))?;
            let kind = entry.file_type()?;
            if kind.is_file()
                && MAC_MEMBERS
                    .iter()
                    .any(|member| relative == Path::new(member))
            {
                members += 1;
            } else if kind.is_dir()
                && MAC_DIRECTORIES
                    .iter()
                    .any(|member| relative == Path::new(member))
            {
                pending.push(path);
            } else {
                return Ok(false);
            }
        }
    }
    Ok(members == MAC_MEMBERS.len())
}

fn replace(replacements: &mut [Replacement], files: &mut dyn FileMoves) -> io::Result<()> {
    for replacement in replacements {
        files.rename(&replacement.installed, &replacement.backup)?;
        replacement.original_moved = true;
        files.rename(&replacement.staged, &replacement.installed)?;
        replacement.replacement_moved = true;
    }
    Ok(())
}

fn rollback(replacements: &mut [Replacement], files: &mut dyn FileMoves) -> bool {
    let mut restored = true;
    for replacement in replacements.iter_mut().rev() {
        if replacement.replacement_moved {
            if files
                .rename(&replacement.installed, &replacement.staged)
                .is_err()
            {
                restored = false;
                continue;
            }
            replacement.replacement_moved = false;
        }
        if replacement.original_moved {
            if files
                .rename(&replacement.backup, &replacement.installed)
                .is_err()
            {
                restored = false;
                continue;
            }
            replacement.original_moved = false;
        }
    }
    restored
}

fn cleanup_empty_backup(backup: &Path) -> io::Result<()> {
    match fs::remove_file(backup.join(READY)) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    // Never recursively delete after rollback: a residual original or unknown
    // file must survive even if an internal bookkeeping invariant was violated.
    fs::remove_dir(backup)
}

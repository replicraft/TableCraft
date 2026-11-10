use std::{
    io,
    path::Path,
    process::{Child, Command, Stdio},
};

#[cfg(any(windows, test))]
use std::path::PathBuf;

#[cfg(any(windows, test))]
use base64::{Engine, engine::general_purpose::STANDARD};
#[cfg(unix)]
use std::os::unix::process::CommandExt;
#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(any(unix, test))]
const UNIX_SCRIPT: &str = include_str!("helper/relaunch.sh");
#[cfg(any(windows, test))]
const WINDOWS_SCRIPT: &str = include_str!("helper/relaunch.ps1");
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Clone, Copy)]
pub(super) enum LaunchKind {
    Portable,
    Bundle,
    AppImage,
}

pub(super) struct HelperPlan<'a> {
    pub(super) process: u32,
    pub(super) launch: &'a Path,
    pub(super) working_directory: &'a Path,
    pub(super) backup: &'a Path,
    pub(super) kind: LaunchKind,
}

pub(super) fn spawn(plan: &HelperPlan<'_>) -> io::Result<Option<Child>> {
    let mut command = command(plan)?;
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(unix)]
    command.process_group(0);
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    command.spawn().map(Some)
}

fn command(plan: &HelperPlan<'_>) -> io::Result<Command> {
    #[cfg(unix)]
    {
        Ok(unix_command(plan))
    }
    #[cfg(windows)]
    {
        let system = std::env::var_os("SystemRoot")
            .ok_or_else(|| io::Error::other("Windows system directory is unavailable"))?;
        let executable =
            PathBuf::from(system).join("System32/WindowsPowerShell/v1.0/powershell.exe");
        Ok(windows_command(plan, executable))
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = plan;
        Err(io::Error::other("Native update helpers are unavailable"))
    }
}

#[cfg(any(unix, test))]
pub(super) fn unix_command(plan: &HelperPlan<'_>) -> Command {
    let mut command = Command::new("/bin/sh");
    command
        .env_remove("ENV")
        .env_remove("BASH_ENV")
        .env_remove("CDPATH")
        .arg("-c")
        .arg(UNIX_SCRIPT)
        .arg("tablecraft-update")
        .arg(plan.process.to_string())
        .arg(plan.launch)
        .arg(plan.working_directory)
        .arg(plan.backup)
        .arg(match plan.kind {
            LaunchKind::Portable => "portable",
            LaunchKind::Bundle => "bundle",
            LaunchKind::AppImage => "appimage",
        });
    command
}

#[cfg(any(windows, test))]
pub(super) fn windows_command(plan: &HelperPlan<'_>, executable: PathBuf) -> Command {
    // PowerShell requires one Base64 argument containing the complete UTF-16LE
    // script. Only fixed source is encoded; application paths remain child env.
    let script: Vec<u8> = WINDOWS_SCRIPT
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    let mut command = Command::new(executable);
    command
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-EncodedCommand",
        ])
        .arg(STANDARD.encode(script))
        .env("TABLECRAFT_UPDATE_PARENT", plan.process.to_string())
        .env("TABLECRAFT_UPDATE_LAUNCH", plan.launch)
        .env(
            "TABLECRAFT_UPDATE_WORKING_DIRECTORY",
            plan.working_directory,
        )
        .env("TABLECRAFT_UPDATE_BACKUP", plan.backup);
    command
}

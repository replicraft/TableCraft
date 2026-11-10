use crate::UpdateError;
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::Value;
use std::{
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const INSPECT: &str = include_str!("native.ps1");
const MAX_REPORT_BYTES: u64 = 1024 * 1024;
const COMMAND_TIMEOUT: Duration = Duration::from_secs(120);
const POLL_INTERVAL: Duration = Duration::from_millis(50);
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub(super) fn inspect(request: &Value, cancelled: &dyn Fn() -> bool) -> Result<Value, UpdateError> {
    if !cfg!(windows) {
        return Err(UpdateError::Archive);
    }
    let executable = powershell()?;
    run(&executable, INSPECT, request, cancelled)
}

pub(super) fn powershell() -> Result<PathBuf, UpdateError> {
    let system = std::env::var_os("SystemRoot").ok_or(UpdateError::Archive)?;
    Ok(PathBuf::from(system).join("System32/WindowsPowerShell/v1.0/powershell.exe"))
}

pub(super) fn command(executable: &Path, script: &str) -> Command {
    let encoded: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let mut command = Command::new(executable);
    command
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-EncodedCommand",
        ])
        .arg(STANDARD.encode(encoded))
        .env_remove("TABLECRAFT_UPDATE_SIGNING_KEY")
        .env_remove("TABLECRAFT_UPDATE_SIGNING_PASSWORD");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

fn run(
    executable: &Path,
    script: &str,
    request: &Value,
    cancelled: &dyn Fn() -> bool,
) -> Result<Value, UpdateError> {
    if cancelled() {
        return Err(UpdateError::Cancelled);
    }
    let output = tempfile::NamedTempFile::new().map_err(UpdateError::Io)?;
    let writer = output.as_file().try_clone().map_err(UpdateError::Io)?;
    let mut child = command(executable, script)
        .stdin(Stdio::piped())
        .stdout(writer)
        .stderr(Stdio::null())
        .spawn()
        .map_err(UpdateError::Io)?;
    let result = (|| {
        let mut input = child.stdin.take().ok_or(UpdateError::Archive)?;
        serde_json::to_writer(&mut input, request).map_err(|_| UpdateError::Archive)?;
        input.flush().map_err(UpdateError::Io)?;
        drop(input);
        let deadline = Instant::now() + COMMAND_TIMEOUT;
        loop {
            if cancelled() {
                return Err(UpdateError::Cancelled);
            }
            if let Some(status) = child.try_wait().map_err(UpdateError::Io)? {
                if !status.success() {
                    return Err(UpdateError::Archive);
                }
                break;
            }
            if Instant::now() >= deadline
                || output.as_file().metadata().map_err(UpdateError::Io)?.len() > MAX_REPORT_BYTES
            {
                return Err(UpdateError::Archive);
            }
            thread::sleep(POLL_INTERVAL);
        }
        let mut bytes = Vec::new();
        File::open(output.path())
            .map_err(UpdateError::Io)?
            .take(MAX_REPORT_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(UpdateError::Io)?;
        if bytes.len() as u64 > MAX_REPORT_BYTES {
            return Err(UpdateError::Archive);
        }
        serde_json::from_slice(&bytes).map_err(|_| UpdateError::Archive)
    })();
    if result.is_err() {
        let _ = child.kill();
        let _ = child.wait();
    }
    result
}

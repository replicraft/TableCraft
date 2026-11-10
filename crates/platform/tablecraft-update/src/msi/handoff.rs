use super::native;
use crate::UpdateError;
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::Value;
use std::{
    fs,
    io::Write,
    path::Path,
    process::{Child, Stdio},
    thread,
    time::{Duration, Instant},
};

const BOOTSTRAP: &str = include_str!("bootstrap.ps1");
const ELEVATED: &str = include_str!("install.ps1");
const HANDOFF_TIMEOUT: Duration = Duration::from_secs(120);
const POLL_INTERVAL: Duration = Duration::from_millis(50);

pub(super) fn spawn(request: &Value) -> Result<Child, UpdateError> {
    if !cfg!(windows) {
        return Err(UpdateError::Archive);
    }
    let script = BOOTSTRAP.replace(
        "__TABLECRAFT_INSTALLER_SOURCE__",
        &STANDARD.encode(ELEVATED.as_bytes()),
    );
    let mut child = native::command(&native::powershell()?, &script)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(UpdateError::Io)?;
    let result = (|| {
        let mut input = child.stdin.take().ok_or(UpdateError::Archive)?;
        serde_json::to_writer(&mut input, request).map_err(|_| UpdateError::Archive)?;
        input.flush().map_err(UpdateError::Io)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = child.kill();
        let _ = child.wait();
    }
    result.map(|()| child)
}

pub(super) fn wait_ready(
    child: &mut Child,
    directory: &Path,
    digest: &str,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), UpdateError> {
    let deadline = Instant::now() + HANDOFF_TIMEOUT;
    loop {
        if cancelled() {
            return Err(UpdateError::Cancelled);
        }
        let marker = directory.join("READY");
        match fs::symlink_metadata(&marker) {
            Ok(metadata) if metadata.is_file() && metadata.len() == digest.len() as u64 => {
                if fs::read(&marker).map_err(UpdateError::Io)? == digest.as_bytes() {
                    return Ok(());
                }
                return Err(UpdateError::PayloadChanged);
            }
            Ok(_) => return Err(UpdateError::PayloadChanged),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(UpdateError::Io(error)),
        }
        if child.try_wait().map_err(UpdateError::Io)?.is_some()
            || directory.join("FAILED.json").exists()
            || Instant::now() >= deadline
        {
            return Err(UpdateError::Archive);
        }
        thread::sleep(POLL_INTERVAL);
    }
}

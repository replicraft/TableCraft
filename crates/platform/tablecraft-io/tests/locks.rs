use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
use tablecraft_io::{ExclusiveFile, IoError, SharedFile};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "tablecraft-lock-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::write(&path, b"original").unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        fs::remove_file(&self.0).unwrap();
    }
}

#[test]
fn independent_handles_share_read_locks_and_exclude_writers_until_drop() {
    let temp = Temp::new();
    let first = SharedFile::open(&temp.0).unwrap();
    let second = SharedFile::open(&temp.0).unwrap();
    assert!(matches!(
        ExclusiveFile::open(&temp.0),
        Err(IoError::LockBusy)
    ));
    drop(first);
    assert!(matches!(
        ExclusiveFile::open(&temp.0),
        Err(IoError::LockBusy)
    ));
    drop(second);
    let exclusive = ExclusiveFile::open(&temp.0).unwrap();
    assert!(matches!(SharedFile::open(&temp.0), Err(IoError::LockBusy)));
    assert!(matches!(
        ExclusiveFile::open(&temp.0),
        Err(IoError::LockBusy)
    ));
    drop(exclusive);
    assert!(SharedFile::open(&temp.0).is_ok());
    assert_eq!(fs::read(&temp.0).unwrap(), b"original");
}

#[test]
fn process_probe() {
    let Ok(path) = std::env::var("TABLECRAFT_LOCK_PROBE_PATH") else {
        return;
    };
    let expected_busy = std::env::var("TABLECRAFT_LOCK_PROBE_BUSY").unwrap() == "yes";
    let result = ExclusiveFile::open(std::path::Path::new(&path));
    if expected_busy {
        assert!(matches!(result, Err(IoError::LockBusy)));
    } else {
        assert!(result.is_ok());
    }
}

#[test]
fn lock_contention_and_release_are_observed_by_another_process() {
    let temp = Temp::new();
    let lock = ExclusiveFile::open(&temp.0).unwrap();
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "process_probe"])
        .env("TABLECRAFT_LOCK_PROBE_PATH", &temp.0)
        .env("TABLECRAFT_LOCK_PROBE_BUSY", "yes")
        .output()
        .unwrap();
    assert!(output.status.success());
    drop(lock);
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "process_probe"])
        .env("TABLECRAFT_LOCK_PROBE_PATH", &temp.0)
        .env("TABLECRAFT_LOCK_PROBE_BUSY", "no")
        .output()
        .unwrap();
    assert!(output.status.success());
}

#[test]
fn failed_open_does_not_create_files_or_expose_host_paths() {
    let temp = Temp::new();
    let missing = temp.0.with_extension("missing");
    let error = SharedFile::open(&missing).err().unwrap();
    assert!(!missing.exists());
    assert!(!error.to_string().contains("tablecraft-lock"));
}

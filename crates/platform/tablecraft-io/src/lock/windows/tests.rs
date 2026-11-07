use super::open_error;
use crate::{ExclusiveFile, IoError, SharedFile};
use std::os::windows::fs::OpenOptionsExt;
use std::{fs, io};

#[test]
fn sharing_and_lock_violations_are_busy_without_hiding_other_errors() {
    // Independent WinError.h inputs: access denied must stay distinct from busy.
    for code in [32, 33] {
        assert_eq!(
            open_error(io::Error::from_raw_os_error(code)),
            IoError::LockBusy
        );
    }
    assert_eq!(
        open_error(io::Error::from_raw_os_error(5)),
        IoError::Io(io::ErrorKind::PermissionDenied)
    );
    assert_eq!(
        open_error(io::Error::from(io::ErrorKind::NotFound)),
        IoError::Io(io::ErrorKind::NotFound)
    );
}

#[test]
fn retained_share_denial_reports_busy_until_the_handle_is_closed() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("target.accdb");
    fs::write(&path, b"original bytes").unwrap();
    let owner = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .share_mode(0)
        .open(&path)
        .unwrap();

    assert!(matches!(SharedFile::open(&path), Err(IoError::LockBusy)));
    assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));

    drop(owner);
    SharedFile::open(&path).unwrap().close().unwrap();
    ExclusiveFile::open(&path).unwrap().close().unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"original bytes");
}

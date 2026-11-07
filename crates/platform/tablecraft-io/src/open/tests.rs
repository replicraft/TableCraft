use super::*;
use std::{fs, io::Read};

#[test]
fn regular_input_retains_bytes_and_portable_missing_file_error() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("input");
    fs::write(&path, b"Id\tScore\n1\t10\n").unwrap();
    let mut file = open_regular_readonly(&path).unwrap();
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"Id\tScore\n1\t10\n");
    assert_eq!(
        open_regular_readonly(&root.path().join("absent"))
            .unwrap_err()
            .kind(),
        io::ErrorKind::NotFound
    );
    assert!(open_regular_readonly(root.path()).is_err());
}

#[cfg(unix)]
fn make_fifo(path: &std::path::Path) {
    assert!(
        std::process::Command::new("mkfifo")
            .arg(path)
            .status()
            .unwrap()
            .success()
    );
}

#[cfg(unix)]
#[test]
fn fifo_without_peer_refuses_after_nonblocking_open() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("fifo");
    make_fifo(&path);
    assert_eq!(
        open_regular_readonly(&path).unwrap_err().kind(),
        io::ErrorKind::InvalidInput
    );
}

#[cfg(unix)]
#[test]
fn symlink_to_regular_input_preserves_existing_open_semantics() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("input");
    let alias = root.path().join("alias");
    fs::write(&path, b"retained").unwrap();
    std::os::unix::fs::symlink(&path, &alias).unwrap();
    let mut file = open_regular_readonly(&alias).unwrap();
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"retained");
}

#[test]
fn locked_regular_opens_preserve_contents_and_existing_symlink_semantics() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("input");
    fs::write(&path, b"original").unwrap();
    {
        let mut shared = crate::SharedFile::open(&path).unwrap();
        let mut bytes = Vec::new();
        shared.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, b"original");
        shared.close().unwrap();
    }
    crate::ExclusiveFile::open(&path).unwrap().close().unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"original");
    #[cfg(unix)]
    {
        let alias = directory.path().join("alias");
        std::os::unix::fs::symlink(&path, &alias).unwrap();
        crate::SharedFile::open(&alias).unwrap().close().unwrap();
        crate::ExclusiveFile::open(&alias).unwrap().close().unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"original");
    }
    assert!(crate::SharedFile::open(directory.path()).is_err());
    assert!(crate::ExclusiveFile::open(directory.path()).is_err());
}

#[cfg(unix)]
#[test]
fn shared_and_exclusive_fifo_opens_refuse_without_waiting_for_a_peer() {
    use rustix::fs::{Mode, OFlags};
    use std::{sync::mpsc, thread, time::Duration};

    for exclusive in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("fifo");
        make_fifo(&path);
        let requested = path.clone();
        let (sender, receiver) = mpsc::sync_channel(1);
        let task = thread::spawn(move || {
            let error = if exclusive {
                crate::ExclusiveFile::open(&requested).err()
            } else {
                crate::SharedFile::open(&requested).err()
            };
            sender.send(error).unwrap();
        });
        let immediate = receiver.recv_timeout(Duration::from_secs(2));
        // If blocking-open regresses, supply a nonblocking peer before joining
        // the worker, so the regression fails without hanging the test process.
        let rescue = if immediate.is_err() {
            Some(
                rustix::fs::open(
                    &path,
                    OFlags::RDWR | OFlags::NONBLOCK | OFlags::CLOEXEC,
                    Mode::empty(),
                )
                .unwrap(),
            )
        } else {
            None
        };
        task.join().unwrap();
        drop(rescue);
        assert_eq!(
            immediate.unwrap(),
            Some(crate::IoError::Io(io::ErrorKind::InvalidInput))
        );
    }
}

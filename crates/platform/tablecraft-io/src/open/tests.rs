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
#[test]
fn fifo_without_peer_refuses_after_nonblocking_open() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("fifo");
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&path)
            .status()
            .unwrap()
            .success()
    );
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

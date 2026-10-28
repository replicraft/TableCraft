use std::{
    fs,
    path::Path,
    process::{Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

const DEADLINE: Duration = Duration::from_secs(5);

fn image() -> Vec<u8> {
    let mut bytes = vec![0; 3 * 4096];
    bytes[..4].copy_from_slice(&[0, 1, 0, 0]);
    bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    bytes[20] = 2;
    bytes[110..114].copy_from_slice(&[8, 31, 152, 253]);
    bytes[62..66].copy_from_slice(&[0xfb, 0x8a, 0xbc, 0x4e]);
    bytes
}

fn command() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_tablecraft"));
    command.env_remove("DISPLAY").env_remove("WAYLAND_DISPLAY");
    command
}

fn open(path: &Path) -> Output {
    command()
        .current_dir(path.parent().unwrap())
        .arg("open")
        .arg("--")
        .arg(path)
        .output()
        .unwrap()
}

#[test]
fn received_paths_preserve_spaces_unicode_and_literal_metacharacters() {
    let directory = tempfile::tempdir().unwrap();
    let bytes = image();
    let names = [
        "space database.accdb",
        "café 数据库.MDB",
        "$(touch marker);`literal`' database.accdb",
    ];
    let quoted = cfg!(unix).then_some("quoted \"database\".mdb");

    for name in names.into_iter().chain(quoted) {
        let path = directory.path().join(name);
        fs::write(&path, &bytes).unwrap();
        let received = open(&path);
        let inspected = command()
            .arg("inspect")
            .arg("--")
            .arg(&path)
            .output()
            .unwrap();
        assert!(received.status.success());
        assert!(received.stderr.is_empty());
        assert_eq!(received.stdout, inspected.stdout);
        assert!(
            String::from_utf8(received.stdout)
                .unwrap()
                .contains("Format\tAce2007")
        );
        assert_eq!(fs::read(&path).unwrap(), bytes);
        fs::remove_file(path).unwrap();
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 0);
    }

    let name = "-leading.accdb";
    let path = directory.path().join(name);
    fs::write(&path, &bytes).unwrap();
    let output = command()
        .current_dir(directory.path())
        .args(["open", "--", name])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(fs::read(path).unwrap(), bytes);
}

#[test]
fn wait_with_piped_streams_returns_without_consuming_open_stdin() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("valid.accdb");
    fs::write(&path, image()).unwrap();

    for (path, expected_status) in [(path, 0), (directory.path().join("missing.accdb"), 2)] {
        let mut child = command()
            .args(["open", "--wait", "--"])
            .arg(path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        // Keep the parent's stdin pipe open: an accidental terminal wait would
        // block rather than receive EOF from wait_with_output's cleanup.
        let stdin = child.stdin.take().unwrap();
        let deadline = Instant::now() + DEADLINE;
        loop {
            if child.try_wait().unwrap().is_some() {
                break;
            }
            if Instant::now() >= deadline {
                child.kill().unwrap();
                let _ = child.wait();
                panic!("open --wait blocked with redirected streams");
            }
            thread::sleep(Duration::from_millis(10));
        }
        drop(stdin);
        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(expected_status));
        assert!(!String::from_utf8_lossy(&output.stderr).contains("Press Enter"));
        if expected_status == 0 {
            assert!(!output.stdout.is_empty());
            assert!(output.stderr.is_empty());
        } else {
            assert!(output.stdout.is_empty());
            assert!(String::from_utf8_lossy(&output.stderr).starts_with("error:"));
        }
    }
}

#[test]
fn malformed_received_file_has_no_success_output_and_help_is_explicit() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("malformed.accdb");
    fs::write(&path, [0; 10]).unwrap();
    let output = open(&path);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(fs::read(path).unwrap(), [0; 10]);

    let help = command().args(["open", "--help"]).output().unwrap();
    assert!(help.status.success());
    let text = String::from_utf8(help.stdout).unwrap();
    assert!(text.contains("--wait"));
    assert!(text.contains("never executed"));
}

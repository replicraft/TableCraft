#![cfg(any(target_os = "macos", target_os = "linux"))]

use sha2::{Digest, Sha256};
use std::{fmt::Write as _, fs, process::Command};
use tablecraft_testkit::access::genuine_retained_numeric as sources;

fn verify(raw: &[u8], expected: &str) {
    let actual = Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(actual, expected);
}

#[test]
#[ignore = "requires all pinned retained numeric originals and expected images"]
fn genuine_retained_numeric_cli_saves_eight_complete_images_and_reads_every_marker() {
    let cases = sources::load_all(verify);
    let mut saved = 0;
    for case in cases {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("target.accdb");
        let journal = root.path().join("journal");
        fs::write(&path, &case.before).unwrap();
        let sql = format!("UPDATE {} SET Marker=42 WHERE [ID]=1", case.table);
        let output = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args(["query", "journal-update", &sql])
            .arg("--database")
            .arg(&path)
            .arg("--journal")
            .arg(&journal)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        assert_eq!(output.stdout, b"Transaction\tCommitted\nMatched rows\t1\nChanged rows\t1\nJournal namespace\tRetired and synchronized\n");
        assert_eq!(fs::read(&path).unwrap(), case.after);
        assert!(!journal.exists());
        let select = format!("SELECT [ID],Marker FROM {}", case.table);
        let read = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args(["query", "run", &select])
            .arg("--database")
            .arg(&path)
            .output()
            .unwrap();
        assert!(
            read.status.success(),
            "{}",
            String::from_utf8_lossy(&read.stderr)
        );
        assert!(read.stderr.is_empty());
        let mut expected = String::from("ID\tMarker\n");
        for id in 1..=case.rows {
            let marker = if id == 1 { 42 } else { id * 17 };
            writeln!(expected, "{id}\t{marker}").unwrap();
        }
        assert_eq!(read.stdout, expected.as_bytes());
        assert_eq!(fs::read(&path).unwrap(), case.after);
        assert!(!journal.exists());
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
        saved += 1;
    }
    assert_eq!(saved, 8);
}

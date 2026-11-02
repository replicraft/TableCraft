#![cfg(any(target_os = "macos", target_os = "linux"))]

use sha2::{Digest, Sha256};
use std::{fs, process::Command};
use tablecraft_testkit::fixtures::access::genuine_ordinary_insert as sources;

fn verify(bytes: &[u8], expected: &str) {
    let actual = Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(actual, expected);
}

#[test]
#[ignore = "requires all four pinned ordinary INSERT SOURCE images"]
fn cli_insert_and_follow_on_noop_use_shared_normal_services_and_exact_rows() {
    for case in sources::load_all(verify) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("target.accdb");
        let bundle = directory.path().join("journal");
        fs::write(&path, &case.before).unwrap();
        for (command, sql, report) in [
            (
                "journal-insert",
                "INSERT INTO Sentinel ([ID], Marker, Bytes) VALUES (24, 'new', NULL)",
                "Transaction\tCommitted\nInserted rows\t1\nJournal namespace\tRetired and synchronized\n",
            ),
            (
                "journal-update",
                "UPDATE Sentinel SET Marker='new' WHERE [ID]=24",
                "Transaction\tCommitted\nMatched rows\t1\nChanged rows\t0\nJournal namespace\tRetired and synchronized\n",
            ),
        ] {
            let output = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
                .args(["query", command, sql])
                .arg("--database")
                .arg(&path)
                .arg("--journal")
                .arg(&bundle)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(output.stderr.is_empty());
            assert_eq!(output.stdout, report.as_bytes());
            assert_eq!(fs::read(&path).unwrap(), case.after);
            assert!(!bundle.exists());
            let read = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
                .args(["query", "run", "SELECT [ID],Marker,Bytes FROM Sentinel"])
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
            assert_eq!(read.stdout,
                b"ID\tMarker\tBytes\n23\t\"\"\"unchanged sentinel\"\"\"\t0xc700ff\n24\t\"\"\"new\"\"\"\tNULL\n");
            assert_eq!(fs::read(&path).unwrap(), case.after);
            assert!(!bundle.exists());
            assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        }
    }
}

#![cfg(any(target_os = "macos", target_os = "linux"))]

use sha2::{Digest, Sha256};
use std::{fs, process::Command};
use tablecraft_testkit::access::genuine_forwarded_delete as sources;

fn verify(bytes: &[u8], expected: &str) {
    let actual = Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(actual, expected);
}

#[test]
#[ignore = "requires pinned EOF and forwarded DELETE SOURCE images"]
fn cli_grow_then_delete_preserves_hidden_bytes_and_reads_zero_live_rows() {
    let cases = sources::load_all(verify);
    let mut saves = 0;
    for case in cases {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("target.accdb");
        let bundle = directory.path().join("journal");
        fs::write(&path, &case.original).unwrap();
        for (command, sql, expected, report, rows) in [
            (
                "journal-update",
                "UPDATE Sentinel SET Marker='aaaaaaaaaaaaaaaaaaa' WHERE [ID]=23",
                &case.before,
                "Transaction\tCommitted\nMatched rows\t1\nChanged rows\t1\nJournal namespace\tRetired and synchronized\n",
                "ID\tMarker\tBytes\n23\t\"\"\"aaaaaaaaaaaaaaaaaaa\"\"\"\t0xc700ff\n",
            ),
            (
                "journal-delete",
                "DELETE FROM Sentinel WHERE [ID]=23",
                &case.after,
                "Transaction\tCommitted\nMatched rows\t1\nDeleted rows\t1\nJournal namespace\tRetired and synchronized\n",
                "ID\tMarker\tBytes\n",
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
            assert_eq!(fs::read(&path).unwrap().as_slice(), expected.as_slice());
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
            assert_eq!(read.stdout, rows.as_bytes());
            assert_eq!(fs::read(&path).unwrap().as_slice(), expected.as_slice());
            assert!(!bundle.exists());
            assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
            saves += 1;
        }
    }
    assert_eq!(saves, 4);
}

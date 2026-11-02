#![cfg(any(target_os = "macos", target_os = "linux"))]

use sha2::{Digest, Sha256};
use std::{fs, process::Command};
use tablecraft_testkit::fixtures::access::genuine_compound_eof as sources;

const SQL: &str =
    "UPDATE Sentinel SET Marker=IIf([ID]=23,'aaaaaaaaaaaaaaaaaaa','ccccccccccccccccccc')";

fn verify(bytes: &[u8], expected: &str) {
    let actual = Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(actual, expected);
}

#[test]
#[ignore = "requires all four source-qualified compound owned-plus-EOF images"]
fn compound_owned_and_eof_cli_commits_both_rows_and_reads_exact_values() {
    let cases = sources::load_all(verify);
    let mut saves = 0;
    for case in cases {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("target.accdb");
        let bundle = directory.path().join("journal");
        fs::write(&path, &case.before).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args(["query", "journal-update", SQL])
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
        assert_eq!(output.stdout, b"Transaction\tCommitted\nMatched rows\t2\nChanged rows\t2\nJournal namespace\tRetired and synchronized\n");
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
        assert_eq!(
            read.stdout,
            b"ID\tMarker\tBytes\n23\t\"\"\"aaaaaaaaaaaaaaaaaaa\"\"\"\t0xc700ff\n24\t\"\"\"ccccccccccccccccccc\"\"\"\t0xc700ff\n"
        );
        assert_eq!(fs::read(&path).unwrap(), case.after);
        assert!(!bundle.exists());
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        saves += 1;
    }
    assert_eq!(saves, 2);
}

#![cfg(any(target_os = "macos", target_os = "linux"))]

use sha2::{Digest, Sha256};
use std::{fs, process::Command};
use tablecraft_testkit::fixtures::access::genuine_ordinary_insert_feff as sources;

fn verify(bytes: &[u8], expected: &str) {
    let actual = Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(actual, expected);
}

#[test]
#[ignore = "requires six independently qualified leading-FEFF INSERT images"]
fn cli_preserves_literal_leading_feff_and_queries_its_actual_character() {
    for case in sources::load_all(verify) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("target.accdb");
        let bundle = directory.path().join("journal");
        fs::write(&path, &case.before).unwrap();
        let written = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args([
                "query",
                "journal-insert",
                "INSERT INTO Sentinel (ID, Marker, Bytes) VALUES (24, '\u{feff}new', NULL)",
            ])
            .arg("--database")
            .arg(&path)
            .arg("--journal")
            .arg(&bundle)
            .output()
            .unwrap();
        assert!(
            written.status.success(),
            "{}",
            String::from_utf8_lossy(&written.stderr)
        );
        assert!(written.stderr.is_empty());
        assert_eq!(written.stdout, b"Transaction\tCommitted\nInserted rows\t1\nJournal namespace\tRetired and synchronized\n");
        assert_eq!(fs::read(&path).unwrap(), case.after);
        assert!(!bundle.exists());

        let read = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args([
                "query",
                "run",
                "SELECT ID, Len(Marker) AS Units, AscW(Left(Marker,1)) AS Head FROM Sentinel WHERE ID=24",
            ])
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
        // FEFF is signed UTF16 code -257; the readable alias has only 3 units.
        assert_eq!(read.stdout, b"ID\tUnits\tHead\n24\t4\t-257\n");
        assert_eq!(fs::read(&path).unwrap(), case.after);
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }
}

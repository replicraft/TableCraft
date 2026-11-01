#![cfg(any(target_os = "macos", target_os = "linux"))]

use sha2::{Digest, Sha256};
use std::{fs, process::Command};
use tablecraft_testkit::access::genuine_owned_growth as sources;

fn verify(raw: &[u8], expected: &str) {
    let actual = Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(actual, expected);
}

#[test]
#[ignore = "requires all pinned original, growth and second-edit source images"]
fn genuine_owned_growth_cli_saves_first_growth_then_fits_same_forwarded_root() {
    let cases = sources::load_all(verify);
    let mut saves = 0;
    for case in cases {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("target.accdb");
        let bundle = root.path().join("journal");
        fs::write(&path, &case.before).unwrap();
        for (marker, expected) in [
            (sources::GROWN, &case.after),
            (sources::SECOND, &case.second),
        ] {
            let sql = format!("UPDATE Sentinel SET Marker='{marker}' WHERE [ID]=23");
            let output = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
                .args(["query", "journal-update", &sql])
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
            assert_eq!(output.stdout, b"Transaction\tCommitted\nMatched rows\t1\nChanged rows\t1\nJournal namespace\tRetired and synchronized\n");
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
            let rows = format!("ID\tMarker\tBytes\n23\t\"\"\"{marker}\"\"\"\t0xc700ff\n");
            assert_eq!(read.stdout, rows.as_bytes());
            assert_eq!(fs::read(&path).unwrap().as_slice(), expected.as_slice());
            assert!(!bundle.exists());
            saves += 1;
        }
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    }
    assert_eq!(saves, 4);
}

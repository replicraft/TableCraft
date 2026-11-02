//! Shared CLI process/readback plumbing; fixture families stay in named tests.
use sha2::{Digest, Sha256};
use std::{fs, path::Path, process::Command};

pub fn verify(raw: &[u8], expected: &str) {
    let actual = Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(actual, expected);
}

pub fn run<'a>(
    binary: &Path,
    cases: impl IntoIterator<Item = (&'a [u8], &'a [u8], &'a [u8])>,
    markers: [&str; 2],
) -> usize {
    let mut saves = 0;
    for (before, grown, second) in cases {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("target.accdb");
        let bundle = root.path().join("journal");
        fs::write(&path, before).unwrap();
        for (marker, expected) in [(markers[0], grown), (markers[1], second)] {
            let sql = format!("UPDATE Sentinel SET Marker='{marker}' WHERE [ID]=23");
            let output = Command::new(binary)
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
            assert_eq!(fs::read(&path).unwrap().as_slice(), expected);
            assert!(!bundle.exists());
            let read = Command::new(binary)
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
            assert_eq!(fs::read(&path).unwrap().as_slice(), expected);
            assert!(!bundle.exists());
            saves += 1;
        }
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    }
    saves
}

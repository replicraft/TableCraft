#![cfg(any(target_os = "macos", target_os = "linux"))]
use sha2::{Digest, Sha256};
use std::{fs, process::Command};
use tablecraft_testkit::access::genuine_wide_update as source;

#[test]
#[ignore = "requires independently qualified wider UPDATE literal sources"]
fn cli_wider_indexed_null_updates_preserve_exact_images_and_readback() {
    let cases = source::load_all(|raw, wanted| {
        assert_eq!(
            Sha256::digest(raw)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            wanted
        );
    });
    let mut saves = 0;
    for case in cases.iter().filter(|case| case.recipe == "present-to-null") {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("target.accdb");
        let journal = directory.path().join("journal");
        fs::write(&path, &case.before).unwrap();
        let sql = format!(
            "UPDATE [{}] SET [KeyValue] = NULL WHERE [Marker] = 17",
            case.table
        );
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

        // Read only stable Int32 Marker plus NULL presence, avoiding a new
        // application rendering oracle for Currency or Int64 primary values.
        let sql = format!(
            "SELECT [Marker], IsNull([KeyValue]) AS Missing FROM [{}]",
            case.table
        );
        let output = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args(["query", "run", &sql])
            .arg("--database")
            .arg(&path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        assert_eq!(
            output.stdout,
            b"Marker\tMissing\n17\tTRUE\n34\tTRUE\n51\tFALSE\n"
        );
        assert_eq!(fs::read(&path).unwrap(), case.after);
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        saves += 1;
    }
    assert_eq!(saves, 8);
}

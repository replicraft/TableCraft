#![cfg(any(target_os = "macos", target_os = "linux"))]
use sha2::{Digest, Sha256};
use std::{fs, process::Command};
use tablecraft_testkit::access::genuine_text_storage_carriers as sources;

#[test]
#[ignore = "requires qualified Text Source, fitting writer and original literal outcomes"]
fn cli_fitting_general_text_updates_preserve_complete_files_and_read_exact_text() {
    let cases = sources::load_all(|bytes, expected| {
        assert_eq!(
            Sha256::digest(bytes)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            expected
        );
    });
    let mut saves = 0;
    for case in cases {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("target.accdb");
        let bundle = directory.path().join("journal");
        fs::write(&path, &case.before).unwrap();
        let sql = format!(
            "UPDATE [{}] SET [KeyValue]='{}' WHERE [ID]=2",
            case.table, case.value
        );
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
        assert_eq!(fs::read(&path).unwrap(), case.after);
        assert!(!bundle.exists());
        let read_sql = format!("SELECT [ID],[KeyValue] FROM [{}] WHERE [ID]=2", case.table);
        let read = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args(["query", "run", &read_sql])
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
            format!("ID\tKeyValue\n2\t\"\"\"{}\"\"\"\n", case.value).as_bytes()
        );
        assert_eq!(fs::read(&path).unwrap(), case.after);
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        saves += 1;
    }
    assert_eq!(saves, 16);
}

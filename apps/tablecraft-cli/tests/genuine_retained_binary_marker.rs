#![cfg(any(target_os = "macos", target_os = "linux"))]
use sha2::{Digest, Sha256};
use std::fs;
use tablecraft_core::values::Value;
use tablecraft_testkit::{
    fixtures::access::genuine_retained_binary_marker::{self as source, Recipe},
    support::cli::Runner,
};

const CLI: Runner = Runner::new(env!("CARGO_BIN_EXE_tablecraft"));

#[test]
#[ignore = "requires six independently qualified retained Binary Marker images"]
fn cli_marker_updates_commit_exact_images_and_read_null_binary_distinctions() {
    let cases = source::load_all(|raw, hash| {
        let actual: String = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(actual, hash);
    });
    let mut saves = 0;
    for case in &cases {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("target.accdb");
        let journal = dir.path().join("journal");
        fs::write(&path, &case.before).unwrap();
        let output = CLI.run(&[
            "query",
            "journal-update",
            case.sql(),
            "--database",
            path.to_str().unwrap(),
            "--journal",
            journal.to_str().unwrap(),
        ]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        let expected = format!(
            "Transaction\tCommitted\nMatched rows\t{}\nChanged rows\t{}\nJournal namespace\tRetired and synchronized\n",
            case.matched(),
            case.changed()
        );
        assert_eq!(output.stdout, expected.as_bytes());
        assert_eq!(fs::read(&path).unwrap(), case.after);
        assert!(!journal.exists());
        let read = CLI.run(&[
            "query",
            "run",
            "SELECT Id, IsNull(KeyValue) AS Missing, Marker FROM BinaryKeys",
            "--database",
            path.to_str().unwrap(),
        ]);
        assert!(
            read.status.success(),
            "{}",
            String::from_utf8_lossy(&read.stderr)
        );
        assert!(read.stderr.is_empty());
        let mut expected = String::from("Id\tMissing\tMarker\n");
        for row in case.rows() {
            let Value::Int32(id) = row[0] else {
                panic!("frozen Id carrier")
            };
            let marker = match row[2] {
                Value::Int32(value) => value.to_string(),
                Value::Null => "NULL".to_owned(),
                _ => panic!("frozen nullable Marker carrier"),
            };
            expected.push_str(&format!(
                "{id}\t{}\t{marker}\n",
                if row[1] == Value::Null {
                    "TRUE"
                } else {
                    "FALSE"
                }
            ));
        }
        assert_eq!(read.stdout, expected.as_bytes());
        assert_eq!(fs::read(&path).unwrap(), case.after);
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
        saves += 1;
    }
    assert_eq!(saves, 8);
}

#[test]
#[ignore = "requires independently qualified original Binary schemas"]
fn cli_selected_primary_and_int32_overflow_refuse_before_output_or_journal_creation() {
    let cases = source::load_all(|raw, hash| {
        let actual: String = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(actual, hash);
    });
    for case in cases.iter().filter(|case| case.recipe == Recipe::Batch) {
        for sql in [
            "UPDATE BinaryKeys SET Id=14 WHERE Id=1",
            // SQL Int32 assignment accepts checked integral carriers, including
            // Int64. This value is out of range; no false exact-carrier claim.
            "UPDATE BinaryKeys SET Marker=2147483648 WHERE Id=1",
        ] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("target.accdb");
            let journal = dir.path().join("journal");
            fs::write(&path, &case.before).unwrap();
            let output = CLI.run(&[
                "query",
                "journal-update",
                sql,
                "--database",
                path.to_str().unwrap(),
                "--journal",
                journal.to_str().unwrap(),
            ]);
            assert_eq!(output.status.code(), Some(2));
            assert!(output.stdout.is_empty());
            assert!(!output.stderr.is_empty());
            assert_eq!(fs::read(&path).unwrap(), case.before);
            assert!(!journal.exists());
            assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
        }
    }
}

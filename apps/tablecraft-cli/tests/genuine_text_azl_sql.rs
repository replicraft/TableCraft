#![cfg(any(target_os = "macos", target_os = "linux"))]

use sha2::{Digest, Sha256};
use std::{fs, process::Command};
use tablecraft_testkit::access::genuine_text_azl::{self as sources, Case};

fn cases() -> Vec<Case> {
    sources::load_all(|raw, expected| {
        let actual = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, expected);
    })
}

#[test]
#[ignore = "requires14 independently qualified explicit-AZL source views"]
fn cli_explicit_property_inserts_read_null_vs_empty_and_retire_source_relative_noops() {
    let mut saves = 0;
    let mut noops = 0;
    for case in cases()
        .into_iter()
        .filter(|case| case.after.is_some() && case.recipe != "empty-insert")
    {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("target.accdb");
        let bundle = directory.path().join("journal");
        fs::write(&path, &case.before).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args(["query", "journal-insert", &case.sql().unwrap()])
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
        assert_eq!(output.stdout, b"Transaction\tCommitted\nInserted rows\t1\nJournal namespace\tRetired and synchronized\n");
        assert_eq!(fs::read(&path).unwrap(), case.after_bytes());
        assert!(!bundle.exists());
        let query = format!(
            "SELECT [ID], IsNull([KeyValue]) AS Missing, Len([KeyValue]) AS Units FROM [{}] WHERE [ID]=4",
            case.table
        );
        let read = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args(["query", "run", &query])
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
            if case.recipe == "insert-empty" {
                b"ID\tMissing\tUnits\n4\tFALSE\t0\n".as_slice()
            } else {
                b"ID\tMissing\tUnits\n4\tTRUE\tNULL\n".as_slice()
            }
        );
        saves += 1;
        let query = format!("DELETE FROM [{}] WHERE [ID]=-1", case.table);
        let empty = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args(["query", "journal-delete", &query])
            .arg("--database")
            .arg(&path)
            .arg("--journal")
            .arg(&bundle)
            .output()
            .unwrap();
        assert!(
            empty.status.success(),
            "{}",
            String::from_utf8_lossy(&empty.stderr)
        );
        assert!(empty.stderr.is_empty());
        assert_eq!(empty.stdout, b"Transaction\tCommitted\nMatched rows\t0\nDeleted rows\t0\nJournal namespace\tRetired and synchronized\n");
        assert_eq!(fs::read(&path).unwrap(), case.after_bytes());
        assert!(!bundle.exists());
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        noops += 1;
    }
    assert_eq!((saves, noops), (12, 12));
}

#[test]
#[ignore = "requires14 independently qualified explicit-AZL source views"]
fn cli_false_azl_reports_field_empty_text_and_creates_no_journal_or_output() {
    let mut denials = 0;
    for case in cases().into_iter().filter(|case| case.after.is_none()) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("target.accdb");
        let bundle = directory.path().join("journal");
        fs::write(&path, &case.before).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args(["query", "journal-insert", &case.sql().unwrap()])
            .arg("--database")
            .arg(&path)
            .arg("--journal")
            .arg(&bundle)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("field disallows zero-length text"),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(fs::read(&path).unwrap(), case.before);
        assert!(!bundle.exists());
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        denials += 1;
    }
    assert_eq!(denials, 4);
}

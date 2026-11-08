#![cfg(any(target_os = "macos", target_os = "linux"))]

use sha2::{Digest, Sha256};
use std::{fs, process::Command};
use tablecraft_testkit::fixtures::access::genuine_text_cardinality::{self as sources, Case};

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
#[ignore = "requires54 independently qualified Text cardinality source views"]
fn cli_text_cardinality_retires_literal_images_and_reads_exact_live_counts() {
    let mut saves = [0; 2];
    for case in cases().into_iter().filter(|case| {
        if case.action == "insert" {
            case.recipe.starts_with("insert-") && case.recipe != "insert-empty"
        } else {
            case.recipe != "delete-exteriors-reverse"
        }
    }) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("target.accdb");
        let bundle = directory.path().join("journal");
        fs::write(&path, &case.before).unwrap();
        let command = if case.action == "insert" {
            "journal-insert"
        } else {
            "journal-delete"
        };
        let output = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args(["query", command, &case.sql().unwrap()])
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
        let counts = if case.action == "insert" {
            saves[0] += 1;
            format!("Inserted rows\t{}\n", case.changed_rows())
        } else {
            saves[1] += 1;
            format!(
                "Matched rows\t{}\nDeleted rows\t{}\n",
                case.changed_rows(),
                case.changed_rows()
            )
        };
        assert_eq!(
            output.stdout,
            format!(
                "Transaction\tCommitted\n\
                    {counts}Journal namespace\tRetired\n\
                    Retirement guarantee\tCaptured-parent barrier\n\
                    Journal cleanup\tRemoved\n"
            )
            .as_bytes()
        );
        assert_eq!(fs::read(&path).unwrap(), case.after);
        assert!(!bundle.exists());
        let query = format!("SELECT Count(*) AS Remaining FROM [{}]", case.table);
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
            format!("Remaining\n{}\n", case.final_rows().len()).as_bytes()
        );
        assert_eq!(fs::read(&path).unwrap(), case.after);
        assert!(!bundle.exists());
    }
    assert_eq!(saves, [12, 32]);
}

#[test]
#[ignore = "requires54 independently qualified Text cardinality source views"]
fn cli_empty_text_insert_has_no_inferred_azl_permission_or_journal() {
    let mut denials = 0;
    for case in cases()
        .into_iter()
        .filter(|case| case.recipe == "insert-empty")
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
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains("ConstraintCoverage"));
        assert_eq!(fs::read(&path).unwrap(), case.before);
        assert!(!bundle.exists());
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        denials += 1;
    }
    assert_eq!(denials, 4);
}

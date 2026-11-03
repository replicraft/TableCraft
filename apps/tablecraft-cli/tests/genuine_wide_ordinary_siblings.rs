#![cfg(any(target_os = "macos", target_os = "linux"))]
use sha2::{Digest, Sha256};
use std::fs;
use tablecraft_core::values::Value;
use tablecraft_testkit::{
    fixtures::access::genuine_wide_ordinary_siblings::{self as source, Recipe},
    support::cli::Runner,
};

const CLI: Runner = Runner::new(env!("CARGO_BIN_EXE_tablecraft"));

#[test]
#[ignore = "requires eighteen independently qualified wider-sibling images"]
fn cli_exact_numeric_zero_null_noop_and_empty_saves_preserve_all_companions() {
    let cases = source::load_all(|raw, hash| {
        let actual: String = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(actual, hash);
    });
    let mut saves = 0;
    for case in cases {
        let Some(sql) = case.sql() else {
            continue;
        };
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("target.accdb");
        let journal = directory.path().join("journal");
        fs::write(&path, &case.before).unwrap();
        let output = CLI.run(&[
            "query",
            "journal-update",
            &sql,
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

        let query = format!(
            "SELECT Id, IsNull(KeyValue) AS MissingKey, Marker, Enabled, Label, IsNull(Opaque) AS MissingBlob FROM [{}]",
            case.table
        );
        let read = CLI.run(&["query", "run", &query, "--database", path.to_str().unwrap()]);
        assert!(
            read.status.success(),
            "{}",
            String::from_utf8_lossy(&read.stderr)
        );
        assert!(read.stderr.is_empty());
        let mut expected = String::from("Id\tMissingKey\tMarker\tEnabled\tLabel\tMissingBlob\n");
        for row in case.rows() {
            let Value::Int32(id) = row[0] else {
                panic!("original Id")
            };
            let Value::Int32(marker) = row[2] else {
                panic!("original Marker")
            };
            let Value::Boolean(enabled) = row[3] else {
                panic!("original Boolean")
            };
            let Value::Text(label) = &row[4] else {
                panic!("original Text")
            };
            expected.push_str(&format!(
                "{id}\t{}\t{marker}\t{}\t\"\"\"{label}\"\"\"\t{}\n",
                if row[1] == Value::Null {
                    "TRUE"
                } else {
                    "FALSE"
                },
                if enabled { "TRUE" } else { "FALSE" },
                if row[5] == Value::Null {
                    "TRUE"
                } else {
                    "FALSE"
                }
            ));
        }
        assert_eq!(read.stdout, expected.as_bytes());
        assert_eq!(fs::read(&path).unwrap(), case.after);
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        saves += 1;
    }
    assert_eq!(saves, 16);
}

#[test]
#[ignore = "requires four independently qualified original numeric companion contexts"]
fn cli_primary_null_duplicate_and_wrong_carrier_refuse_before_output_or_journal() {
    let cases = source::load_all(|raw, hash| {
        let actual: String = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(actual, hash);
    });
    let mut refusals = 0;
    for case in cases.into_iter().filter(|case| case.recipe == Recipe::Zero) {
        for assignment in ["Id=NULL", "Id=2", "KeyValue='wrong'"] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("target.accdb");
            let journal = directory.path().join("journal");
            fs::write(&path, &case.before).unwrap();
            let sql = format!("UPDATE [{}] SET {} WHERE Id=1", case.table, assignment);
            let output = CLI.run(&[
                "query",
                "journal-update",
                &sql,
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
            assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
            refusals += 1;
        }
    }
    assert_eq!(refusals, 12);
}

#[test]
#[ignore = "requires qualified wider-sibling zero UPDATE images"]
fn cli_small_integer_literal_zero_assigns_to_wider_fields_without_carrier_expressions() {
    let cases = source::load_all(|raw, hash| {
        let actual: String = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(actual, hash);
    });
    let mut saves = 0;
    for case in cases.into_iter().filter(|case| case.recipe == Recipe::Zero) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("target.accdb");
        let journal = directory.path().join("journal");
        fs::write(&path, &case.before).unwrap();
        let sql = format!("UPDATE [{}] SET KeyValue=0 WHERE Id=1", case.table);
        let output = CLI.run(&[
            "query",
            "journal-update",
            &sql,
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
        assert_eq!(output.stdout, b"Transaction\tCommitted\nMatched rows\t1\nChanged rows\t1\nJournal namespace\tRetired and synchronized\n");
        assert_eq!(fs::read(&path).unwrap(), case.after);
        assert!(!journal.exists());

        let sql = format!("SELECT Marker FROM [{}] WHERE KeyValue=0", case.table);
        let read = CLI.run(&["query", "run", &sql, "--database", path.to_str().unwrap()]);
        assert!(
            read.status.success(),
            "{}",
            String::from_utf8_lossy(&read.stderr)
        );
        assert!(read.stderr.is_empty());
        assert_eq!(read.stdout, b"Marker\n17\n");
        assert_eq!(fs::read(&path).unwrap(), case.after);
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        saves += 1;
    }
    assert_eq!(saves, 4);
}

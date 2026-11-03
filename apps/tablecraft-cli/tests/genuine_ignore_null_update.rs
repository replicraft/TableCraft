#![cfg(any(target_os = "macos", target_os = "linux"))]
use sha2::{Digest, Sha256};
use std::fs;
use tablecraft_core::values::Value;
use tablecraft_testkit::{
    fixtures::access::genuine_ignore_null_update::{self as source, Case, Recipe},
    support::cli::Runner,
};

const CLI: Runner = Runner::new(env!("CARGO_BIN_EXE_tablecraft"));

fn cases() -> Vec<Case> {
    source::load_all(|raw, expected| {
        let actual: String = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(actual, expected);
    })
}

#[test]
#[ignore = "requires held genuine IgnoreNULL original/AFTER images"]
fn cli_single_omission_changes_and_distinct_noops_commit_exact_bytes_and_readback() {
    let mut saves = 0;
    for case in cases() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("target.accdb");
        let journal = directory.path().join("journal");
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
            "SELECT ID, [Left], [Right], Marker FROM [Single]",
            "--database",
            path.to_str().unwrap(),
        ]);
        assert!(
            read.status.success(),
            "{}",
            String::from_utf8_lossy(&read.stderr)
        );
        assert!(read.stderr.is_empty());
        let mut expected = String::from("ID\tLeft\tRight\tMarker\n");
        for row in case.rows() {
            let line: Vec<_> = row
                .into_iter()
                .map(|value| match value {
                    Value::Null => "NULL".to_owned(),
                    Value::Int32(value) => value.to_string(),
                    _ => panic!("outside held nullable I32 schema"),
                })
                .collect();
            expected.push_str(&line.join("\t"));
            expected.push('\n');
        }
        assert_eq!(read.stdout, expected.as_bytes());
        assert_eq!(fs::read(&path).unwrap(), case.after);
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        saves += 1;
    }
    assert_eq!(saves, 12);
}

#[test]
#[ignore = "requires the held RequiredSingle and UniqueSingle originals"]
fn cli_ignore_null_does_not_bypass_required_or_unique_constraints() {
    let mut refusals = 0;
    for case in cases()
        .into_iter()
        .filter(|case| case.recipe == Recipe::NullNew)
    {
        for refusal in [source::Refusal::Required, source::Refusal::Unique] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("target.accdb");
            let journal = directory.path().join("journal");
            fs::write(&path, &case.before).unwrap();
            let output = CLI.run(&[
                "query",
                "journal-update",
                refusal.sql().unwrap(),
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
    assert_eq!(refusals, 4);
}

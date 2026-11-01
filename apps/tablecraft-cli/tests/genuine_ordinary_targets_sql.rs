#![cfg(any(target_os = "macos", target_os = "linux"))]

use sha2::{Digest, Sha256};
use std::{
    fmt::Write as _,
    fs,
    path::Path,
    process::{Command, Output},
};
use tablecraft_core::values::Value;
use tablecraft_testkit::access::genuine_ordinary_targets::{self as sources, Recipe};

fn update(path: &Path, journal: &Path, sql: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tablecraft"))
        .args(["query", "journal-update", sql])
        .arg("--database")
        .arg(path)
        .arg("--journal")
        .arg(journal)
        .output()
        .unwrap()
}

fn sql(recipe: Recipe) -> &'static str {
    match recipe {
        Recipe::ScoreSeventeen => "UPDATE Items SET Score=17 WHERE Id=1",
        Recipe::ScoreClear => "UPDATE Items SET Score=NULL WHERE Id=1",
        Recipe::ScoreNoop => "UPDATE Items SET Score=-1 WHERE Id=1",
        Recipe::ScoreEmpty => "UPDATE Items SET Score=17 WHERE Id=-1",
        Recipe::ManualIdSeven => "UPDATE Items SET Id=7 WHERE Id=1",
        _ => panic!("the CLI uses only actual snapshot-resolved contexts"),
    }
}

fn rows(values: &[Vec<Value>]) -> String {
    let mut result = String::from("Id\tScore\tGuard\n");
    for row in values {
        assert_eq!(row.len(), 3);
        for (column, value) in row.iter().enumerate() {
            if column != 0 {
                result.push('\t');
            }
            match value {
                Value::Null => result.push_str("NULL"),
                Value::Int32(value) => write!(result, "{value}").unwrap(),
                _ => panic!("the frozen source has only nullable Int32 user fields"),
            }
        }
        result.push('\n');
    }
    result
}

fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[test]
#[ignore = "requires explicitly supplied source-qualified genuine artifact directory"]
fn genuine_cli_commits_ten_literal_images_and_reads_exact_physical_row_order() {
    let mut saves = 0;
    for header in [5, 6] {
        for recipe in sources::POSITIVES {
            let case = sources::source(header, recipe);
            let after = case.after.as_ref().unwrap();
            assert_eq!(hash(&case.before), case.before_digest);
            assert_eq!(hash(after), case.after_digest.unwrap());
            let expected_rows = rows(&case.final_rows);
            let report = format!(
                "Transaction\tCommitted\nMatched rows\t{}\nChanged rows\t{}\nJournal namespace\tRetired and synchronized\n",
                usize::from(!case.empty),
                case.changed_rows
            );
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join("target.accdb");
            let journal = root.path().join("journal");
            fs::write(&path, &case.before).unwrap();
            let output = update(&path, &journal, sql(recipe));
            assert!(
                output.status.success(),
                "{}: {}",
                recipe.id(),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(output.stderr.is_empty());
            assert_eq!(output.stdout, report.as_bytes());
            assert_eq!(fs::read(&path).unwrap().as_slice(), after.as_slice());
            assert!(!journal.exists());
            assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
            let read = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
                .arg("rows")
                .arg(&path)
                .args(["Items", "--format", "tsv"])
                .output()
                .unwrap();
            assert!(
                read.status.success(),
                "{}",
                String::from_utf8_lossy(&read.stderr)
            );
            assert!(read.stderr.is_empty());
            assert_eq!(read.stdout, expected_rows.as_bytes());
            assert_eq!(fs::read(&path).unwrap().as_slice(), after.as_slice());
            assert!(!journal.exists());
            saves += 1;
        }
    }
    assert_eq!(saves, 10);
}

#[test]
#[ignore = "requires explicitly supplied source-qualified genuine artifact directory"]
fn genuine_cli_duplicate_keys_refuse_before_journal_publication() {
    for header in [5, 6] {
        let case = sources::source(header, Recipe::DuplicateId);
        assert_eq!(hash(&case.before), case.before_digest);
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("target.accdb");
        let journal = root.path().join("journal");
        fs::write(&path, &case.before).unwrap();
        let output = update(&path, &journal, "UPDATE Items SET Id=2 WHERE Id=1");
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert_eq!(
            output.stderr,
            b"error: offline write Planning failed: update backend rejected the candidate: native index constraint violated: duplicate non-NULL key\n",
        );
        assert_eq!(fs::read(&path).unwrap(), case.before);
        assert!(!journal.exists());
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    }
}

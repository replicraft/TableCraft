#![cfg(any(target_os = "macos", target_os = "linux"))]

use std::{
    fmt::Write as _,
    fs,
    path::Path,
    process::{Command, Output},
};
use tablecraft_core::values::Value;
use tablecraft_testkit::fixtures::access::canonical_ordinary_indexed_targets::{
    self as sources, Recipe,
};

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
        Recipe::ManualSeven
        | Recipe::Aliases
        | Recipe::ManualTree
        | Recipe::PresentBesideNull
        | Recipe::GeneratedTarget => "UPDATE Items SET Id=7 WHERE Id=1",
        Recipe::PromoteNull => "UPDATE Items SET Id=2 WHERE Id IS NULL",
        Recipe::ClearValue | Recipe::PrimaryNull => "UPDATE Items SET Id=NULL WHERE Id=1",
        Recipe::UniqueSwap => "UPDATE Items SET Id=3-Id WHERE Id=1 OR Id=2",
        Recipe::UniqueAllNull => "UPDATE Items SET Id=NULL WHERE Id=1 OR Id=2",
        // Every literal and intermediate stays Int32, including the minimum.
        Recipe::PrefixLoss => "UPDATE Items SET Id=1-2147483647-2 WHERE Id=1",
        Recipe::Empty => "UPDATE Items SET Id=7 WHERE Id=-1",
        Recipe::PresentNoop => "UPDATE Items SET Id=1 WHERE Id=1",
        Recipe::NullNoop => "UPDATE Items SET Id=NULL WHERE Id IS NULL",
        Recipe::UniqueConflict => "UPDATE Items SET Id=2 WHERE Id=1",
        Recipe::StaleNullExpected => panic!("SQL cannot supply a forged snapshot expected value"),
    }
}

fn rows(rows: &[Vec<Value>]) -> String {
    let mut output = String::from("Id\tScore\n");
    for row in rows {
        assert_eq!(row.len(), 2);
        for (column, value) in row.iter().enumerate() {
            if column != 0 {
                output.push('\t');
            }
            match value {
                Value::Null => output.push_str("NULL"),
                Value::Int32(value) => write!(output, "{value}").unwrap(),
                _ => panic!("the frozen source has only nullable Int32 fields"),
            }
        }
        output.push('\n');
    }
    output
}

#[test]
fn ordinary_indexed_cli_publishes_twenty_four_literal_outcomes_and_exact_rows() {
    let mut saves = 0;
    for header in [5, 6] {
        for recipe in sources::RECIPES {
            let case = sources::source(header, recipe);
            let Some(expected) = &case.expected else {
                continue;
            };
            let expected_rows = rows(&case.final_rows);
            let report = format!(
                "Transaction\tCommitted\n\
                    Matched rows\t{}\n\
                    Changed rows\t{}\n\
                    Journal namespace\tRetired\n\
                    Retirement guarantee\tCaptured-parent barrier\n\
                    Journal cleanup\tRemoved\n",
                case.updates.len(),
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
            assert_eq!(&fs::read(&path).unwrap(), expected, "{}", recipe.id());
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
                "{}: {}",
                recipe.id(),
                String::from_utf8_lossy(&read.stderr)
            );
            assert!(read.stderr.is_empty());
            assert_eq!(read.stdout, expected_rows.as_bytes());
            assert_eq!(&fs::read(&path).unwrap(), expected);
            assert!(!journal.exists());
            saves += 1;
        }
    }
    assert_eq!(saves, 24);
}

#[test]
fn actual_sql_refusals_and_zero_match_generated_targets_publish_nothing() {
    let mut refused = 0;
    for header in [5, 6] {
        for recipe in [
            Recipe::UniqueConflict,
            Recipe::GeneratedTarget,
            Recipe::PrimaryNull,
        ] {
            let case = sources::source(header, recipe);
            let statements = if recipe == Recipe::GeneratedTarget {
                vec![sql(recipe), "UPDATE Items SET Id=7 WHERE Id=-1"]
            } else {
                vec![sql(recipe)]
            };
            for statement in statements {
                let root = tempfile::tempdir().unwrap();
                let path = root.path().join("target.accdb");
                let journal = root.path().join("journal");
                fs::write(&path, &case.before).unwrap();
                let output = update(&path, &journal, statement);
                assert!(!output.status.success(), "{}", recipe.id());
                assert!(output.stdout.is_empty());
                assert!(!output.stderr.is_empty());
                assert_eq!(fs::read(&path).unwrap(), case.before);
                assert!(!journal.exists());
                assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
                refused += 1;
            }
        }
    }
    assert_eq!(refused, 8);
}

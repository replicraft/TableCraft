#![cfg(any(target_os = "macos", target_os = "linux"))]

use std::{
    fmt::Write as _,
    fs,
    path::Path,
    process::{Command, Output},
};
use tablecraft_core::values::Value;
use tablecraft_testkit::access::canonical_ordinary_fitted::{self as sources, Recipe};

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

fn sql(recipe: Recipe) -> Option<&'static str> {
    match recipe {
        Recipe::NullTextToEmpty => Some("UPDATE Items SET TextValue='' WHERE Id=2"),
        Recipe::EmptyTextToNull => Some("UPDATE Items SET TextValue=NULL WHERE Id=3"),
        Recipe::EmptyBinaryToNull => Some("UPDATE Items SET BinaryValue=NULL WHERE Id=3"),
        Recipe::CompressedText => Some("UPDATE Items SET TextValue='beta' WHERE Id=1"),
        Recipe::RawUnicode => Some("UPDATE Items SET TextValue='Ω雪🙂a' WHERE Id=1"),
        Recipe::EscapedBom => Some("UPDATE Items SET TextValue='\u{feff}' WHERE Id=1"),
        Recipe::ExactNoops => {
            Some("UPDATE Items SET TextValue=TextValue,BinaryValue=BinaryValue WHERE Id=1")
        }
        Recipe::NullNoops => Some("UPDATE Items SET TextValue=NULL,BinaryValue=NULL WHERE Id=2"),
        Recipe::EmptyGroups => {
            Some("UPDATE Items SET TextValue=TextValue,BinaryValue=BinaryValue WHERE Id=-1")
        }
        _ => None,
    }
}

fn projected_rows(rows: &[Vec<Value>]) -> String {
    let mut output = String::from("Id\tTextValue\tBinaryValue\n");
    for row in rows {
        assert_eq!(row.len(), 8);
        let Value::Int32(id) = &row[0] else {
            panic!("literal ordinary ID")
        };
        write!(output, "{id}\t").unwrap();
        match &row[1] {
            Value::Null => output.push_str("NULL"),
            Value::Text(text) => {
                // Display Text retains literal quotes. TSV then quotes that
                // field and doubles its two quote characters, yielding three
                // quotes at each end for this finite quote-free corpus.
                assert!(
                    !text
                        .chars()
                        .any(|character| character.is_control() || matches!(character, '"' | '\\'))
                );
                write!(output, "\"\"\"{text}\"\"\"").unwrap();
            }
            _ => panic!("literal Text carrier"),
        }
        output.push('\t');
        match &row[2] {
            Value::Null => output.push_str("NULL"),
            Value::Binary(bytes) => {
                output.push_str("0x");
                for byte in bytes {
                    write!(output, "{byte:02x}").unwrap();
                }
            }
            _ => panic!("literal Binary carrier"),
        }
        output.push('\n');
    }
    output
}

#[test]
fn ordinary_fitted_cli_saves_and_reads_eighteen_literal_sql_images() {
    let mut saves = 0;
    for header in [5, 6] {
        for recipe in sources::RECIPES {
            let Some(statement) = sql(recipe) else {
                continue;
            };
            let case = sources::source(header, recipe);
            let expected = case.expected.as_ref().unwrap();
            let expected_matched = usize::from(recipe != Recipe::EmptyGroups);
            let report = format!(
                "Transaction\tCommitted\nMatched rows\t{expected_matched}\nChanged rows\t{}\nJournal namespace\tRetired and synchronized\n",
                case.changed,
            );
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join("target.accdb");
            let journal = root.path().join("journal");
            fs::write(&path, &case.before).unwrap();
            let output = update(&path, &journal, statement);
            assert!(
                output.status.success(),
                "{}: {}",
                recipe.id(),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(output.stderr.is_empty());
            assert_eq!(output.stdout, report.as_bytes());
            assert_eq!(fs::read(&path).unwrap().as_slice(), expected.as_slice());
            assert!(!journal.exists());
            let read = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
                .args(["query", "run", "SELECT Id,TextValue,BinaryValue FROM Items"])
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
            assert_eq!(read.stdout, projected_rows(&case.final_rows).as_bytes());
            assert_eq!(fs::read(&path).unwrap().as_slice(), expected.as_slice());
            assert!(!journal.exists());
            assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
            saves += 1;
        }
    }
    assert_eq!(saves, 18);
}

#[test]
fn ordinary_fitted_cli_capacity_refuses_before_bundle_publication() {
    for header in [5, 6] {
        let case = sources::source(header, Recipe::TextOverflow);
        assert!(case.expected.is_none());
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("target.accdb");
        let journal = root.path().join("journal");
        fs::write(&path, &case.before).unwrap();
        let output = update(
            &path,
            &journal,
            "UPDATE Items SET TextValue='雪雪' WHERE Id=3",
        );
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert_eq!(output.stderr, b"error: offline write Planning failed: update backend rejected the candidate: native write profile is unsupported (RowLayout)\n");
        assert_eq!(fs::read(&path).unwrap(), case.before);
        assert!(!journal.exists());
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    }
}

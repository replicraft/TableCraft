#![cfg(any(target_os = "macos", target_os = "linux"))]

use sha2::{Digest, Sha256};
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};
use tablecraft_core::values::Value;
use tablecraft_testkit::fixtures::access::genuine_property_lval::{self as sources, Case, Recipe};

fn verify(bytes: &[u8], expected: &str) {
    let actual = Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(actual, expected);
}

fn update(path: &Path, bundle: &Path, sql: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tablecraft"))
        .args(["query", "journal-update", sql])
        .arg("--database")
        .arg(path)
        .arg("--journal")
        .arg(bundle)
        .output()
        .unwrap()
}

fn sql(recipe: Recipe) -> &'static str {
    match recipe {
        Recipe::Same => "UPDATE Sentinel SET Marker='qualified sentinel' WHERE [ID]=23",
        Recipe::Shorter => "UPDATE Sentinel SET Marker='changed sentinel' WHERE [ID]=23",
        Recipe::Empty => "UPDATE Sentinel SET Marker='' WHERE [ID]=23",
        Recipe::Clear => "UPDATE Sentinel SET Marker=NULL WHERE [ID]=23",
        Recipe::Noop => "UPDATE Sentinel SET Marker=Marker WHERE [ID]=23",
        Recipe::EmptyGroup => "UPDATE Sentinel SET Marker=Marker WHERE [ID]=-1",
        _ => panic!("only the six independently frozen SQL-positive contexts"),
    }
}

fn expected_rows(case: &Case) -> String {
    let marker = if case.empty {
        &case.expected
    } else {
        &case.replacement
    };
    let marker = match marker {
        Value::Null => "NULL".to_owned(),
        Value::Text(value) => {
            assert!(
                !value
                    .chars()
                    .any(|character| character.is_control() || matches!(character, '"' | '\\'))
            );
            // Value display includes quotes; TSV's CSV transport quotes the
            // field and doubles those quotes. This finite corpus needs exactly
            // three quote characters at each end, including present-empty.
            format!("\"\"\"{value}\"\"\"")
        }
        _ => panic!("the literal Marker requests use Text or NULL"),
    };
    format!("ID\tMarker\tBytes\n23\t{marker}\t0xc700ff\n")
}

#[test]
#[ignore = "requires the frozen original and twelve literal expectation files"]
fn genuine_property_lval_cli_saves_and_reads_twelve_literal_images() {
    let cases = sources::load_all(verify);
    let mut saves = 0;
    for case in cases.iter().filter(|case| case.after.is_some()) {
        let after = case.after.as_ref().unwrap();
        let report = format!(
            "Transaction\tCommitted\n\
                Matched rows\t{}\n\
                Changed rows\t{}\n\
                Journal namespace\tRetired\n\
                Retirement guarantee\tCaptured-parent barrier\n\
                Journal cleanup\tRemoved\n",
            usize::from(!case.empty),
            case.changed_rows
        );
        let rows = expected_rows(case);
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("target.accdb");
        let bundle = root.path().join("journal");
        fs::write(&path, &case.before).unwrap();
        let output = update(&path, &bundle, sql(case.recipe));
        assert!(
            output.status.success(),
            "h{} {}: {}",
            case.header,
            case.recipe.id(),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        assert_eq!(output.stdout, report.as_bytes());
        assert_eq!(fs::read(&path).unwrap().as_slice(), after.as_slice());
        assert_eq!(
            &after[case.property_offset..case.property_offset + 57],
            &case.before[case.property_offset..case.property_offset + 57]
        );
        assert!(!bundle.exists());
        let read = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args(["query", "run", "SELECT [ID],Marker,Bytes FROM Sentinel"])
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
        assert_eq!(read.stdout, rows.as_bytes());
        assert_eq!(fs::read(&path).unwrap().as_slice(), after.as_slice());
        assert!(!bundle.exists());
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
        saves += 1;
    }
    assert_eq!(saves, 12);
}

#[test]
#[ignore = "requires the frozen original and twelve literal expectation files"]
fn genuine_property_lval_cli_declared_width_refuses_without_journal_or_file_changes() {
    let cases = sources::load_all(verify);
    let mut refusals = 0;
    for case in cases.iter().filter(|case| case.recipe == Recipe::Overflow) {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("target.accdb");
        let bundle = root.path().join("journal");
        fs::write(&path, &case.before).unwrap();
        let output = update(
            &path,
            &bundle,
            "UPDATE Sentinel SET Marker='aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa' WHERE [ID]=23",
        );
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert_eq!(output.stderr, b"error: offline write Planning failed: update backend rejected the candidate: native write profile is unsupported (Column)\n");
        assert_eq!(fs::read(&path).unwrap(), case.before);
        assert!(!bundle.exists());
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
        refusals += 1;
    }
    assert_eq!(refusals, 2);
}

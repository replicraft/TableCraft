//! CLI consumers of the independently qualified catalog/payload originals.
#[path = "../../../../../crates/engines/tablecraft-ace/src/preservation/table_extra/tests/observed_macro_inventory/coverage_sources/model.rs"]
mod model;

use super::model as observed;
use csv::ReaderBuilder;
use std::{fs, io::Cursor, path::Path, process::Command};
use tablecraft_ace::NativeImage;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

fn inspect(path: &Path, table: &str, extra: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_tablecraft"))
        .args(["--format", "tsv", "inspect"])
        .arg(path)
        .args(["--table", table, "--macro-coverage"])
        .args(extra)
        .output()
        .unwrap()
}

fn rows(bytes: &[u8]) -> Vec<Vec<String>> {
    ReaderBuilder::new()
        .delimiter(b'\t')
        .has_headers(false)
        .flexible(true)
        .from_reader(bytes)
        .records()
        .map(|row| row.unwrap().iter().map(str::to_owned).collect())
        .collect()
}

fn contains(rows: &[Vec<String>], expected: &[&str]) -> bool {
    rows.iter()
        .any(|row| row.iter().map(String::as_str).eq(expected.iter().copied()))
}

fn assert_boundaries(rows: &[Vec<String>], stdout: &[u8]) {
    assert!(contains(
        rows,
        &["Coverage scope", "Table and direct catalog children"]
    ));
    assert!(contains(
        rows,
        &["Required macro coverage", "Not certified"]
    ));
    assert!(contains(rows, &["Data-macro absence", "Not certified"]));
    assert!(contains(rows, &["Save permission", "Not granted"]));
    let text = std::str::from_utf8(stdout).unwrap();
    for body in [
        "<DataMacro",
        "Statements",
        "FutureAction",
        "opaque root",
        "opaque child",
        "unrelated body",
    ] {
        assert!(!text.contains(body), "Stored body escaped into output");
    }
}

#[test]
fn macro_coverage_cli_retains_all_twenty_observed_envelope_sources() {
    for code in [5, 6] {
        for case in observed::CASES {
            let source = observed::source(code, case);
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("source.accdb");
            fs::write(&path, &source.bytes).unwrap();
            let result = inspect(&path, "Items", &[]);
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            let output = rows(&result.stdout);
            assert_boundaries(&output, &result.stdout);
            assert!(contains(&output, &["Direct children", "0"]));
            assert!(contains(&output, &["Table", "10", "Lv", "NULL", ""]));
            assert!(contains(&output, &["Table", "10", "LvModule", "NULL", ""]));
            let length = source.bag.len().to_string();
            assert!(contains(
                &output,
                &["Table", "10", "LvExtra", "Present", &length]
            ));
            for expected in super::expected(case) {
                assert!(contains(&output, &expected));
            }
            assert_eq!(fs::read(&path).unwrap(), source.bytes);
        }
    }
}

#[test]
fn macro_coverage_cli_keeps_missing_null_empty_and_child_payloads_distinct() {
    let scope = CatalogScope::new(19);
    for code in [5, 6] {
        for case in model::CASES {
            let source = model::source(code, case);
            let image = NativeImage::capture(
                &mut Cursor::new(&source.bytes),
                scope,
                &mut ResourceBudget::new(128 * 1024 * 1024, 16_384, 100_000_000),
                &|| false,
            )
            .unwrap();
            let table = image
                .catalog()
                .catalog()
                .get(scope.object_id(source.id))
                .unwrap();
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("source.accdb");
            fs::write(&path, &source.bytes).unwrap();
            let result = inspect(&path, table.name().as_str(), &[]);
            if matches!(case, model::Case::DuplicateRootNames) {
                assert!(!result.status.success());
                assert!(result.stdout.is_empty());
                assert_eq!(fs::read(&path).unwrap(), source.bytes);
                continue;
            }
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            let output = rows(&result.stdout);
            assert_boundaries(&output, &result.stdout);
            let id = source.id.to_string();
            let children = source.children.len().to_string();
            assert!(contains(&output, &["Direct children", &children]));
            for (carrier, payload) in [
                ("Lv", &source.body),
                ("LvModule", &source.module),
                ("LvExtra", &source.extra),
            ] {
                let presence = if source.missing_columns {
                    "Column missing"
                } else if payload.is_some() {
                    "Present"
                } else {
                    "NULL"
                };
                let length = payload
                    .as_ref()
                    .map(|bytes| bytes.len().to_string())
                    .unwrap_or_default();
                assert!(contains(
                    &output,
                    &["Table", &id, carrier, presence, &length]
                ));
            }
            for child in &source.children {
                let id = child.id.to_string();
                let presence = match child.presence {
                    model::Presence::Missing => "Column missing",
                    model::Presence::Absent => "NULL",
                    model::Presence::Present => "Present",
                };
                for (carrier, payload) in [
                    ("Lv", &child.body),
                    ("LvModule", &child.module),
                    ("LvExtra", &child.extra),
                ] {
                    let length = payload
                        .as_ref()
                        .map(|bytes| bytes.len().to_string())
                        .unwrap_or_default();
                    assert!(contains(
                        &output,
                        &["Direct child", &id, carrier, presence, &length]
                    ));
                }
            }
            assert_eq!(fs::read(&path).unwrap(), source.bytes);
        }
    }
}

#[test]
fn macro_coverage_cli_rejects_conflicts_and_exhaustion_without_partial_stdout() {
    let source = observed::source(6, observed::Case::NonstandardEventProperty);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("source.accdb");
    fs::write(&path, &source.bytes).unwrap();
    for extra in [
        vec!["--extra"],
        vec!["--features"],
        vec!["--macro-inventory"],
        vec!["--macro-program"],
        vec!["--macro-xml"],
        vec!["--native-bytes", "1"],
        vec!["--native-steps", "1"],
    ] {
        let result = inspect(&path, "Items", &extra);
        assert!(
            !result.status.success(),
            "Conflicting or exhausted mode unexpectedly succeeded: {extra:?}"
        );
        assert!(result.stdout.is_empty());
        assert_eq!(fs::read(&path).unwrap(), source.bytes);
    }
    let missing = inspect(&path, "NoSuchTable", &[]);
    assert!(!missing.status.success());
    assert!(missing.stdout.is_empty());
}

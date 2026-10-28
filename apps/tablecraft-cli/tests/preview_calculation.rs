#[allow(dead_code)]
#[path = "../../../crates/engines/tablecraft-ace/tests/support/agile.rs"]
mod agile_fixture;
#[allow(dead_code)]
#[path = "../../../crates/engines/tablecraft-ace/tests/calculated_storage/fixture.rs"]
mod calculated_fixture;
#[path = "support/mod.rs"]
mod cli_support;
#[allow(dead_code)]
#[path = "../../../crates/engines/tablecraft-ace/tests/mutation/fixture.rs"]
mod fixture;
use tablecraft_testkit::access as support;

#[path = "preview_calculation/iif.rs"]
mod iif;

use cli_support::run;
use std::{fs, path::PathBuf, process::Output};

const FORMAT_OFFSET: usize = 20;
const PAGE_BYTES: usize = 4096;
const DATA_PAGE: usize = 12;
const FIRST_ROW_SLOT: usize = 14;
const CACHE_WRAPPER_OFFSET: usize = 6;

struct Database {
    _directory: tempfile::TempDir,
    path: PathBuf,
    bytes: Vec<u8>,
}

impl Database {
    fn new(bytes: Vec<u8>) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("preview.accdb");
        fs::write(&path, &bytes).unwrap();
        Self {
            _directory: directory,
            path,
            bytes,
        }
    }

    fn path(&self) -> &str {
        self.path.to_str().unwrap()
    }

    fn preview(&self, extra: &[&str]) -> Output {
        let mut args = vec![
            "--format",
            "tsv",
            "query",
            "preview-calculation",
            "--database",
            self.path(),
            "Items",
            "Score",
        ];
        args.extend_from_slice(extra);
        run(&args)
    }
}

impl Drop for Database {
    fn drop(&mut self) {
        assert_eq!(fs::read(&self.path).unwrap(), self.bytes);
    }
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn explicit_preview_recomputes_stale_null_caches_without_changing_read_policy() {
    for profile in [5, 6] {
        let mut bytes = calculated_fixture::database(&[Some(999), None, Some(41)]);
        bytes[FORMAT_OFFSET] = profile;
        let database = Database::new(bytes);
        let output = database.preview(&[]);
        assert_success(&output);
        assert_eq!(output.stdout, b"Recomputed\n2\n4\n6\n");

        let cached = run(&[
            "--format",
            "tsv",
            "query",
            "run",
            "SELECT Score FROM Items",
            "--database",
            database.path(),
            "--stored-calculations",
        ]);
        assert_success(&cached);
        assert_eq!(cached.stdout, b"Score\n999\nNULL\n41\n");
        let ordinary = run(&[
            "query",
            "run",
            "SELECT Score FROM Items",
            "--database",
            database.path(),
        ]);
        assert!(!ordinary.status.success());
        assert!(ordinary.stdout.is_empty());
    }
}

#[test]
fn unselected_corrupt_cache_does_not_become_a_recomputation_dependency() {
    let mut bytes = calculated_fixture::database(&[Some(999)]);
    let page = DATA_PAGE * PAGE_BYTES;
    let row = u16::from_le_bytes(
        bytes[page + FIRST_ROW_SLOT..page + FIRST_ROW_SLOT + 2]
            .try_into()
            .unwrap(),
    ) as usize;
    bytes[page + row + CACHE_WRAPPER_OFFSET] = 1;
    let database = Database::new(bytes);
    let output = database.preview(&[]);
    assert_success(&output);
    assert_eq!(output.stdout, b"Recomputed\n2\n");
    let stored = run(&[
        "query",
        "run",
        "SELECT Score FROM Items",
        "--database",
        database.path(),
        "--stored-calculations",
    ]);
    assert!(!stored.status.success());
    assert!(stored.stdout.is_empty());
}

#[test]
fn late_arithmetic_failures_never_publish_a_successful_prefix() {
    let database = Database::new(calculated_fixture::database_with_expression(
        &[Some(999), Some(999)],
        "1/([Id]-2)",
    ));
    let output = database.preview(&[]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("calculation preview"));
}

#[test]
fn active_nonlocal_and_self_dependencies_remain_refused() {
    for expression in [
        "Now()",
        "Shell('never')",
        "[Score]+1",
        "[Other].[Id]",
        "[Missing]+1",
    ] {
        let database = Database::new(calculated_fixture::database_with_expression(
            &[Some(999)],
            expression,
        ));
        let output = database.preview(&[]);
        assert!(!output.status.success(), "{expression}");
        assert!(output.stdout.is_empty(), "{expression}");
        assert!(String::from_utf8_lossy(&output.stderr).contains("row-local"));
    }
}

#[test]
fn numeric_text_policies_and_natural_results_remain_explicit() {
    let database = Database::new(calculated_fixture::database_with_expression(
        &[Some(999)],
        "'row=' & [Id]",
    ));
    let failed = database.preview(&[]);
    assert!(!failed.status.success());
    assert!(failed.stdout.is_empty());
    let output = database.preview(&["--number-format", "invariant"]);
    assert_success(&output);
    // The native declaration is Int32; preview output follows the expression.
    assert_eq!(output.stdout, b"Recomputed\n\"\"\"row=1\"\"\"\n");
}

#[test]
fn preview_limits_and_transport_formatting_remain_buffered() {
    let database = Database::new(calculated_fixture::database(&[Some(999), None, Some(41)]));
    for controls in [
        ["--native-bytes", "1"],
        ["--native-rows", "1"],
        ["--native-steps", "1"],
        ["--execution-bytes", "1"],
        ["--execution-rows", "1"],
        ["--execution-steps", "1"],
        ["--output-bytes", "4"],
    ] {
        let output = database.preview(&controls);
        assert!(!output.status.success(), "{}", controls[0]);
        assert!(output.stdout.is_empty(), "{}", controls[0]);
    }
    let output = database.preview(&["--format", "table"]);
    assert_success(&output);
    assert!(String::from_utf8_lossy(&output.stdout).contains("Recomputed"));
    assert!(String::from_utf8_lossy(&output.stdout).contains('│'));

    let text = Database::new(calculated_fixture::database_with_expression(
        &[Some(999)],
        "'abc' & 'def'",
    ));
    let output = text.preview(&["--text-bytes", "5"]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
}

#[test]
fn encrypted_preview_uses_global_password_options_and_keeps_ciphertext_unchanged() {
    let plain = calculated_fixture::database(&[Some(999), None]);
    let password = "public preview password";
    let database = Database::new(agile_fixture::encrypted_image(&plain, 6, password));
    for controls in [vec![], vec!["-p", "incorrect"]] {
        let output = database.preview(&controls);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(!String::from_utf8_lossy(&output.stderr).contains(password));
    }
    for flag in ["-p", "--password"] {
        let output = database.preview(&[flag, password]);
        assert_success(&output);
        assert_eq!(output.stdout, b"Recomputed\n2\n4\n");
    }
}

#[test]
fn preview_help_and_required_arguments_expose_the_separate_preview_boundary() {
    let output = run(&["query", "preview-calculation", "--help"]);
    assert_success(&output);
    let help = String::from_utf8(output.stdout).unwrap();
    for expected in [
        "Recomputed",
        "without",
        "caches",
        "--database",
        "--password",
        "--execution-bytes",
        "--output-bytes",
    ] {
        assert!(help.contains(expected), "{expected}");
    }
    let missing = run(&["query", "preview-calculation", "Items", "Score"]);
    assert!(!missing.status.success());
    assert!(missing.stdout.is_empty());
}

#[test]
fn unsupported_formula_admission_precedes_invalid_user_records() {
    for (expression, unsupported) in [("Now()", true), ("[Id]*2", false)] {
        let mut bytes = calculated_fixture::database_with_expression(&[Some(999)], expression);
        let page = DATA_PAGE * PAGE_BYTES;
        let row = u16::from_le_bytes(
            bytes[page + FIRST_ROW_SLOT..page + FIRST_ROW_SLOT + 2]
                .try_into()
                .unwrap(),
        ) as usize;
        bytes[page + row..page + row + 2].copy_from_slice(&0_u16.to_le_bytes());
        let database = Database::new(bytes);
        let output = database.preview(&[]);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert_eq!(
            String::from_utf8_lossy(&output.stderr).contains("row-local"),
            unsupported
        );
    }
}

#[test]
fn transport_expansion_respects_the_same_output_ceiling() {
    let database = Database::new(calculated_fixture::database_with_expression(
        &[Some(999)],
        "'abc'",
    ));
    // The internal Value display fits 17 bytes. CSV/TSV quote escaping needs
    // another four bytes, which must fail before the transport is published.
    let output = database.preview(&["--output-bytes", "17"]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let output = database.preview(&["--output-bytes", "21"]);
    assert_success(&output);
    assert_eq!(output.stdout, b"Recomputed\n\"\"\"abc\"\"\"\n");
}

#[test]
fn predicate_previews_keep_natural_values_and_buffer_late_eager_failures() {
    for (expression, expected) in [
        ("[Id]=2", "Recomputed\nFALSE\nTRUE\nFALSE\n"),
        ("([Id]>=2) Or False", "Recomputed\nFALSE\nTRUE\nTRUE\n"),
        ("Not [Id]", "Recomputed\n-2\n-3\n-4\n"),
    ] {
        let database = Database::new(calculated_fixture::database_with_expression(
            &[Some(999), Some(999), Some(999)],
            expression,
        ));
        let output = database.preview(&[]);

        assert_success(&output);
        assert_eq!(output.stdout, expected.as_bytes());
    }

    let database = Database::new(calculated_fixture::database_with_expression(
        &[Some(999), Some(999), Some(999)],
        "False And (1/([Id]-2)=0)",
    ));
    let output = database.preview(&[]);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("division by zero"));
}

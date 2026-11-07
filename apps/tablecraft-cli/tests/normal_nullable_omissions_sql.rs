#![cfg(any(target_os = "macos", target_os = "linux", windows))]
//! SQL and desktop consumers share omission semantics, including real native NULLs.
use std::{ffi::OsString, fs, path::Path, process::Output};
use tablecraft_testkit::{
    fixtures::access::{
        canonical_creation_properties::{Policy, with_policies},
        canonical_initial_table_creation::nullable_companions,
    },
    support::cli::Runner,
};

const CLI: Runner = Runner::new(env!("CARGO_BIN_EXE_tablecraft"));
const INSERT: &str = "INSERT INTO Items ([Score]) VALUES (CLng(17))";

fn insert(path: &Path, journal: &Path) -> Output {
    CLI.run_os([
        OsString::from("query"),
        OsString::from("journal-insert"),
        OsString::from(INSERT),
        OsString::from("--database"),
        path.as_os_str().to_owned(),
        OsString::from("--journal"),
        journal.as_os_str().to_owned(),
    ])
}

#[test]
fn partial_sql_rows_resolve_missing_nullable_defaults_to_null_without_coercing_supplied_values() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("optional-界.accdb");
    let journal = directory.path().join("insert.journal");
    let before = nullable_companions().bytes;
    fs::write(&path, &before).unwrap();

    let saved = insert(&path, &journal);
    assert!(
        saved.status.success(),
        "{}",
        String::from_utf8_lossy(&saved.stderr)
    );
    assert!(saved.stderr.is_empty());
    assert!(
        saved
            .stdout
            .starts_with(b"Transaction\tCommitted\nInserted rows\t1\n")
    );
    assert!(!journal.exists());
    let after = fs::read(&path).unwrap();
    assert_ne!(after, before);

    let rows = CLI.run_os([
        OsString::from("query"),
        OsString::from("run"),
        OsString::from("SELECT [Id], [Score], [Name] FROM Items WHERE [Name] IS NULL"),
        OsString::from("--database"),
        path.as_os_str().to_owned(),
    ]);
    assert!(
        rows.status.success(),
        "{}",
        String::from_utf8_lossy(&rows.stderr)
    );
    assert!(rows.stderr.is_empty());
    assert_eq!(rows.stdout, b"Id\tScore\tName\n1\t17\tNULL\n");
    assert_eq!(fs::read(&path).unwrap(), after);
    assert!(!journal.exists());
}

#[test]
fn omitted_required_field_without_a_default_refuses_before_journal_or_target_changes() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("required.accdb");
    let journal = directory.path().join("insert.journal");
    let before = with_policies(
        nullable_companions().bytes,
        &[Policy {
            column: "Name",
            required: Some(true),
            allow_zero_length: None,
        }],
    );
    fs::write(&path, &before).unwrap();

    let refused = insert(&path, &journal);
    assert_eq!(refused.status.code(), Some(2));
    assert!(refused.stdout.is_empty());
    let diagnostic = String::from_utf8(refused.stderr).unwrap();
    assert!(
        diagnostic.contains("required field is NULL"),
        "{diagnostic}"
    );
    assert_eq!(fs::read(&path).unwrap(), before);
    assert!(!journal.exists());
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
}

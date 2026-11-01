#![cfg(any(target_os = "macos", target_os = "linux"))]

use std::{
    fs,
    path::Path,
    process::{Command, Output},
};
use tablecraft_testkit::access::canonical_native17_sessions;

fn action(path: &Path, bundle: &Path, verb: &str, sql: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tablecraft"))
        .args(["query", verb, sql])
        .arg("--database")
        .arg(path)
        .arg("--journal")
        .arg(bundle)
        .output()
        .unwrap()
}

#[test]
fn cli_fresh_create_then_normal_insert_and_delete_match_literal_images_and_rows() {
    let expected = canonical_native17_sessions::source();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("created.accdb");
    let data = directory.path().join("rows.tsv");
    let bundle = directory.path().join("journal");
    fs::write(&data, "Id\tScore\n1\t10\n").unwrap();
    let created = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
        .arg("create")
        .arg(&path)
        .args(["--table", "Items", "--column", "Id", "--column", "Score"])
        .arg("--data")
        .arg(&data)
        .output()
        .unwrap();
    assert!(
        created.status.success(),
        "{}",
        String::from_utf8_lossy(&created.stderr)
    );
    assert!(created.stderr.is_empty());
    assert_eq!(created.stdout, b"Database\tCreated ACE 2016\nColumns\t2\nRows\t1\nDurable output\tFile and captured-directory barriers completed\n");
    assert_eq!(fs::read(&path).unwrap(), expected.before);
    for (verb, sql, after, report, rows) in [
        (
            "journal-insert",
            "INSERT INTO Items ([Id], [Score]) VALUES (2, 20)",
            &expected.inserted,
            "Transaction\tCommitted\nInserted rows\t1\nJournal namespace\tRetired and synchronized\n",
            "Id\tScore\n1\t10\n2\t20\n",
        ),
        (
            "journal-delete",
            "DELETE FROM Items WHERE [Id]=2",
            &expected.deleted,
            "Transaction\tCommitted\nMatched rows\t1\nDeleted rows\t1\nJournal namespace\tRetired and synchronized\n",
            "Id\tScore\n1\t10\n",
        ),
    ] {
        let output = action(&path, &bundle, verb, sql);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        assert_eq!(output.stdout, report.as_bytes());
        assert_eq!(fs::read(&path).unwrap(), *after);
        assert!(!bundle.exists());
        let read = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args(["query", "run", "SELECT [Id], [Score] FROM Items"])
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
        assert_eq!(fs::read(&path).unwrap(), *after);
    }
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 2);
}

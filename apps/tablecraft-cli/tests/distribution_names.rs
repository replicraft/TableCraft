//! Filename routing evidence, not native compiled-project qualification.
#[allow(dead_code)]
#[path = "../../../crates/engines/tablecraft-ace/tests/mutation/fixture.rs"]
mod fixture;
use tablecraft_testkit::access as support;

use std::{fs, path::Path, process::Command};

fn run(path: &Path, command: &[&str], options: &[&str]) -> Vec<u8> {
    let output = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
        .args(command)
        .arg(path)
        .args(options)
        .output()
        .unwrap();
    assert!(output.status.success(), "{:?}", output.stderr);
    assert!(output.stderr.is_empty());
    output.stdout
}

#[test]
fn runtime_and_compiled_names_use_content_for_table_reads_and_queries() {
    let directory = tempfile::tempdir().unwrap();
    let jet = fixture::database();
    let ace = fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/examples/Access_Example_VBA.accdb"),
    )
    .unwrap();
    for (extension, bytes, table) in [
        ("mde", &jet, "Items"),
        ("accdr", &ace, "ROULETTE_TB"),
        ("accde", &ace, "ROULETTE_TB"),
        ("ACCDE", &ace, "ROULETTE_TB"),
    ] {
        let path = directory.path().join(format!("database.{extension}"));
        fs::write(&path, bytes).unwrap();
        let listing = run(&path, &["tables"], &["--count"]);
        assert!(String::from_utf8(listing).unwrap().contains(table));
        let rows = run(&path, &["rows"], &[table, "--limit", "2"]);
        let sql = format!("SELECT TOP 2 * FROM [{table}]");
        assert_eq!(rows, run(&path, &["query", "run", &sql, "--database"], &[]));
        assert!(!run(&path, &["inspect"], &[]).is_empty());
        assert_eq!(fs::read(path).unwrap(), *bytes);
    }
}

#[test]
fn access_suffix_does_not_turn_a_package_or_other_file_into_a_database() {
    let directory = tempfile::tempdir().unwrap();
    for extension in ["accdr", "accde", "mde", "accdt", "accdc"] {
        let path = directory.path().join(format!("invalid.{extension}"));
        fs::write(&path, b"not a native database").unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .arg("tables")
            .arg(&path)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert_eq!(fs::read(path).unwrap(), b"not a native database");
    }
}

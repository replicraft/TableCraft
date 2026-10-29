//! SQL-selected user workflows over the independent engineering source owner.
#[path = "../../../crates/engines/tablecraft-ace/tests/support/agile.rs"]
mod encrypted_fixture;
#[path = "../../../crates/engines/tablecraft-ace/tests/mutation/fixture.rs"]
mod fixture;
#[path = "../../../crates/backends/tablecraft-access/src/engineering/editor/tests/source.rs"]
mod source;
use tablecraft_testkit::access as support;

use std::{
    fs,
    io::Cursor,
    path::Path,
    process::{Command, Output},
};
use tablecraft_ace::{
    EncryptedSource, IndexCheckStatus, check_table, check_table_constraints, read_table,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

fn rows(bytes: &[u8]) -> Vec<Vec<Value>> {
    read_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap()
    .into_parts()
    .2
}

fn run(sql: &str, input: &Path, output: &Path, flags: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tablecraft"))
        .args(["query", "preview-update", sql, "--database"])
        .arg(input)
        .arg("--output")
        .arg(output)
        .args(flags)
        .output()
        .unwrap()
}

fn success(result: Output, matched: usize, changed: usize) {
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(result.stderr.is_empty());
    let report = String::from_utf8(result.stdout).unwrap();
    assert!(report.contains("Experimental"));
    assert!(report.contains(&format!("Matched rows\t{matched}\n")));
    assert!(report.contains(&format!("Candidate rows\t{changed}\n")));
}

#[test]
fn qualify_sql_preview_cli_source_before_any_new_action() {
    let bytes = source::source();
    let table = check_table(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(table.rows(), 4);
    assert!(
        table
            .indexes()
            .iter()
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );
    assert_eq!(
        check_table_constraints(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false
        )
        .unwrap()
        .rows(),
        4
    );
    assert_eq!(
        rows(&bytes)[2],
        vec![
            Value::Int32(3),
            Value::Text("A".to_owned()),
            Value::Int32(30),
            Value::Text("Z".to_owned())
        ]
    );
    let encrypted = encrypted_fixture::encrypted_image(&bytes, 5, "public test password");
    let mut decoded = EncryptedSource::open(
        Cursor::new(&encrypted),
        Some("public test password"),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        read_table(
            &mut decoded,
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false
        )
        .unwrap()
        .rows(),
        rows(&bytes)
    );
}

#[test]
fn sql_text_numeric_null_and_noop_copies_match_selected_original_rows() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("input.accdb");
    let before = source::source();
    fs::write(&input, &before).unwrap();
    let output = directory.path().join("names.accdb");
    success(
        run(
            "UPDATE Items AS i SET i.Label='RENAMED' WHERE i.Label='A'",
            &input,
            &output,
            &["--variable", "--index-growth", "--collation", "ordinal"],
        ),
        2,
        2,
    );
    let after = rows(&fs::read(&output).unwrap());
    assert_eq!(after[0][1], Value::Text("RENAMED".to_owned()));
    assert_eq!(after[1][1], Value::Text("B".to_owned()));
    assert_eq!(after[2][1], Value::Text("RENAMED".to_owned()));
    assert_eq!(after[3][1], Value::Text("C".to_owned()));
    assert_eq!(
        after.iter().map(|row| row[3].clone()).collect::<Vec<_>>(),
        rows(&before)
            .iter()
            .map(|row| row[3].clone())
            .collect::<Vec<_>>()
    );
    for (file, literal, expected) in [
        ("cleared.accdb", "NULL", Value::Null),
        (
            "literal-null.accdb",
            "'null'",
            Value::Text("null".to_owned()),
        ),
    ] {
        let path = directory.path().join(file);
        success(
            run(
                &format!("UPDATE Items SET Label={literal} WHERE Id=2"),
                &input,
                &path,
                &["--variable"],
            ),
            1,
            1,
        );
        assert_eq!(rows(&fs::read(path).unwrap())[1][1], expected);
    }
    let scores = directory.path().join("scores.accdb");
    success(
        run(
            "UPDATE Items SET Score=11 WHERE Id>=2",
            &input,
            &scores,
            &[],
        ),
        3,
        3,
    );
    let after = rows(&fs::read(scores).unwrap());
    assert_eq!(after[0][2], Value::Int32(10));
    assert!(after[1..].iter().all(|row| row[2] == Value::Int32(11)));
    let noop = directory.path().join("noop.accdb");
    success(
        run(
            "UPDATE Items SET Score=Score WHERE Id<=2",
            &input,
            &noop,
            &[],
        ),
        2,
        0,
    );
    assert_eq!(fs::read(noop).unwrap(), before);
    assert_eq!(fs::read(input).unwrap(), before);
}

#[test]
fn sql_profile_validation_precedes_open_and_late_errors_publish_no_copy() {
    let directory = tempfile::tempdir().unwrap();
    let missing = directory.path().join("missing.accdb");
    let output = directory.path().join("refused.accdb");
    for sql in [
        "PARAMETERS P LONG; UPDATE Items SET Score=P",
        "UPDATE Items SET Score=1,Label='x'",
        "UPDATE Items IN 'external.accdb' SET Score=11",
    ] {
        let result = run(sql, &missing, &output, &[]);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        assert!(!String::from_utf8_lossy(&result.stderr).contains("NotFound"));
        assert!(!output.exists());
    }
    let before = source::source();
    let input = directory.path().join("input.accdb");
    fs::write(&input, &before).unwrap();
    for (sql, flags) in [
        ("UPDATE Items SET Score=10 \\ (4-Id)", vec![]),
        (
            "UPDATE Items SET Score=11 WHERE Id<0 AND (1 \\ 0)=0",
            vec![],
        ),
        ("UPDATE Items SET Label=11", vec!["--variable"]),
        (
            "UPDATE Items SET Label='bad mode' WHERE Id<0",
            vec!["--long-value"],
        ),
        (
            "UPDATE Items SET Label='no implicit collation' WHERE Label='A'",
            vec!["--variable"],
        ),
    ] {
        let result = run(sql, &input, &output, &flags);
        assert_eq!(
            result.status.code(),
            Some(2),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(result.stdout.is_empty());
        assert!(!output.exists());
        assert_eq!(fs::read(&input).unwrap(), before);
    }
    fs::write(&output, b"existing destination").unwrap();
    assert_eq!(
        run("UPDATE Items SET Score=11", &input, &output, &[])
            .status
            .code(),
        Some(2)
    );
    assert_eq!(fs::read(output).unwrap(), b"existing destination");
    let normal = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
        .args(["query", "update", "UPDATE Items SET Score=11", "--database"])
        .arg(&input)
        .arg("--output")
        .arg(directory.path().join("normal.accdb"))
        .output()
        .unwrap();
    assert_eq!(normal.status.code(), Some(2));
    assert!(!directory.path().join("normal.accdb").exists());
    assert_eq!(fs::read(input).unwrap(), before);
}

#[test]
fn sql_encrypted_sources_and_conflicting_modes_refuse_without_plaintext_output() {
    let directory = tempfile::tempdir().unwrap();
    let before = source::source();
    let ciphertext = encrypted_fixture::encrypted_image(&before, 5, "public test password");
    let input = directory.path().join("encrypted.accdb");
    let output = directory.path().join("refused.accdb");
    fs::write(&input, &ciphertext).unwrap();
    let result = run(
        "UPDATE Items SET Score=11",
        &input,
        &output,
        &["--password", "public test password"],
    );
    assert_eq!(result.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&result.stderr).contains("encrypted rewrite"));
    assert!(result.stdout.is_empty());
    assert!(!output.exists());
    for flags in [
        vec!["--variable", "--long-value"],
        vec!["--index-growth", "--cascade-updates"],
    ] {
        let result = run("UPDATE Items SET Score=11", &input, &output, &flags);
        assert_eq!(result.status.code(), Some(2));
        assert!(!output.exists());
    }
    assert_eq!(fs::read(input).unwrap(), ciphertext);
}

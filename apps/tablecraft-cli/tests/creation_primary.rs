#![cfg(any(target_os = "macos", target_os = "linux"))]

use std::{
    fs,
    io::Cursor,
    path::Path,
    process::{Command, Output},
};
use tablecraft_ace::{IndexCheckStatus, check_table, read_table, read_table_index};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access::{
    canonical_creation_integral_primary as wide, canonical_creation_primary as i32_sources,
};

const SCOPE: CatalogScope = CatalogScope::new(225);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn create(
    path: &Path,
    data: Option<&Path>,
    kind: Option<&str>,
    selector: &str,
    index_name: &str,
) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_tablecraft"));
    command
        .args(["create"])
        .arg(path)
        .args(["--table", "Items"]);
    if let Some(kind) = kind {
        command.args(["--field", "Id", kind, "--field", "Score", "int32"]);
    } else {
        command.args(["--column", "Id", "--column", "Score"]);
    }
    command.args(["--primary-key", selector, index_name]);
    if let Some(data) = data {
        command.arg("--data").arg(data);
    }
    command.output().unwrap()
}

fn token(value: &Value) -> String {
    match value {
        Value::Null => "null".into(),
        Value::Int32(value) => value.to_string(),
        Value::Int64(value) => value.to_string(),
        Value::Currency(value) => value.scaled().to_string(),
        _ => panic!("literal exact integral source"),
    }
}

fn data(rows: &[Vec<Value>]) -> String {
    let mut result = String::from("Id\tScore\n");
    for row in rows {
        result.push_str(&token(&row[0]));
        result.push('\t');
        result.push_str(&token(&row[1]));
        result.push('\n');
    }
    result
}

fn key(row: &[Value]) -> i64 {
    match row[0] {
        Value::Int32(value) => i64::from(value),
        Value::Int64(value) => value,
        Value::Currency(value) => value.scaled(),
        _ => panic!("healthy present source key"),
    }
}

fn readback(rows: &[Vec<Value>]) -> String {
    let mut expected = String::from("Id\tScore\n");
    for row in rows {
        let visible = match row[0] {
            Value::Int32(value) => value.to_string(),
            Value::Int64(value) => value.to_string(),
            Value::Currency(value) => match value.scaled() {
                i64::MIN => "-922337203685477.5808".into(),
                i64::MAX => "922337203685477.5807".into(),
                0 => "0.0000".into(),
                -1 => "-0.0001".into(),
                _ => panic!("literal Currency source endpoint"),
            },
            _ => panic!("healthy exact source key"),
        };
        let Value::Int32(score) = row[1] else {
            panic!("literal Int32 Score");
        };
        expected.push_str(&format!("{visible}\t{score}\n"));
    }
    expected
}

fn success(expected: &[u8], rows: &[Vec<Value>], kind: Option<&str>) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("created.accdb");
    let input = directory.path().join("rows.tsv");
    let supplied = if rows.is_empty() {
        None
    } else {
        fs::write(&input, data(rows)).unwrap();
        Some(input.as_path())
    };
    let result = create(&path, supplied, kind, "id", "PrimaryId");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(result.stderr.is_empty());
    assert_eq!(result.stdout, format!(
        "Database\tCreated ACE 2016\nColumns\t2\nRows\t{}\nDurable output\tFile and captured-directory barriers completed\n",
        rows.len(),
    ).as_bytes());
    let raw = fs::read(&path).unwrap();
    assert_eq!(raw, expected);
    let actual = read_table(
        &mut Cursor::new(&raw),
        SCOPE,
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(actual.rows(), rows);
    let checked = check_table(
        &mut Cursor::new(&raw),
        SCOPE,
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(checked.indexes().len(), 1);
    assert_eq!(checked.indexes()[0].status(), IndexCheckStatus::Checked);
    assert_eq!(checked.indexes()[0].index().name(), "PrimaryId");
    assert_eq!(checked.schema().columns()[0].flags(), 3);
    assert_eq!(&expected[7 * 4096 + 20..7 * 4096 + 24], &[0; 4]);
    let mut ordered = rows.to_vec();
    ordered.sort_by_key(|row| key(row));
    assert_eq!(
        read_table_index(
            &mut Cursor::new(&raw),
            SCOPE,
            "Items",
            "PrimaryId",
            &mut budget(),
            &|| false,
        )
        .unwrap()
        .rows(),
        ordered
    );
    let read = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
        .arg("rows")
        .arg(&path)
        .arg("Items")
        .output()
        .unwrap();
    assert!(
        read.status.success(),
        "{}",
        String::from_utf8_lossy(&read.stderr)
    );
    assert!(read.stderr.is_empty());
    assert_eq!(read.stdout, readback(rows).as_bytes());
    assert_eq!(fs::read(&path).unwrap(), expected);
    assert_eq!(
        fs::read_dir(directory.path()).unwrap().count(),
        1 + usize::from(supplied.is_some())
    );
}

#[test]
fn explicit_primary_in_both_cli_schema_modes_matches_twelve_whole_sources_and_readbacks() {
    let mut outputs = 0;
    for case in i32_sources::CASES {
        let source = i32_sources::source(case);
        if source.violation.is_some() {
            continue;
        }
        for kind in [None, Some("int32")] {
            success(&source.bytes, &source.rows, kind);
            outputs += 1;
        }
    }
    for case in wide::CASES {
        let source = wide::source(case);
        if source.violation.is_some() {
            continue;
        }
        let kind = match source.family {
            wide::Family::Int64 => "int64",
            wide::Family::Currency => "currency-scaled",
        };
        success(&source.bytes, &source.rows, Some(kind));
        outputs += 1;
    }
    assert_eq!(outputs, 12);
}

#[test]
fn caller_named_primary_is_forwarded_and_existing_destination_is_preserved() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("created.accdb");
    let input = directory.path().join("rows.tsv");
    fs::write(&input, "Id\tScore\n1\t10\n").unwrap();
    let result = create(&path, Some(&input), None, "Id", "ChosenKey");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(result.stderr.is_empty());
    let raw = fs::read(&path).unwrap();
    let checked = check_table(
        &mut Cursor::new(&raw),
        SCOPE,
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(checked.indexes()[0].index().name(), "ChosenKey");
    assert_eq!(checked.indexes()[0].index().columns()[0].number(), 0);
    let again = create(&path, Some(&input), None, "Id", "OtherKey");
    assert!(!again.status.success());
    assert!(again.stdout.is_empty());
    assert_eq!(fs::read(&path).unwrap(), raw);
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 2);
}

#[test]
fn bad_selector_kind_name_duplicate_or_null_keys_publish_no_file() {
    let mut refusals = 0;
    for (kind, selector, index, rows, diagnostic) in [
        (
            Some("int32"),
            "Missing",
            "PrimaryId",
            "Id\tScore\n1\t10\n",
            "does not match a declared column",
        ),
        (
            Some("byte"),
            "Id",
            "PrimaryId",
            "Id\tScore\n1\t10\n",
            "declaration",
        ),
        (
            Some("int32"),
            "Id",
            "Bad.Name",
            "Id\tScore\n1\t10\n",
            "valid native identifier",
        ),
        (
            None,
            "Id",
            "PrimaryId",
            "Id\tScore\n1\t10\n1\t20\n",
            "duplicate non-NULL key",
        ),
        (
            Some("int32"),
            "Id",
            "PrimaryId",
            "Id\tScore\nnull\t10\n",
            "NULL key in primary or required index",
        ),
        (
            Some("int64"),
            "Id",
            "PrimaryId",
            "Id\tScore\n-9223372036854775808\t10\n-9223372036854775808\t20\n",
            "duplicate non-NULL key",
        ),
        (
            Some("currency-scaled"),
            "Id",
            "PrimaryId",
            "Id\tScore\nnull\t10\n",
            "NULL key in primary or required index",
        ),
        (
            None,
            "Id",
            "PrimaryId",
            "Id\tScore\nnull\t10\n",
            "present decimal Int32",
        ),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("absent.accdb");
        let input = directory.path().join("rows.tsv");
        fs::write(&input, rows).unwrap();
        let result = create(&path, Some(&input), kind, selector, index);
        assert_eq!(
            result.status.code(),
            Some(2),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(result.stdout.is_empty());
        assert!(
            String::from_utf8_lossy(&result.stderr)
                .to_lowercase()
                .contains(&diagnostic.to_lowercase())
        );
        assert!(!path.exists());
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        refusals += 1;
    }
    assert_eq!(refusals, 8);
}

#![cfg(any(target_os = "macos", target_os = "linux"))]

use sha2::{Digest, Sha256};
use std::{fs, process::Command};
use tablecraft_core::values::Value;
use tablecraft_testkit::fixtures::access::genuine_integral_indexed::{self as sources, Case};

fn verify(raw: &[u8], expected: &str) {
    let hash = Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(hash, expected);
}

fn sql(case: &Case) -> String {
    if case.operation == "insert" {
        assert_eq!(case.recipe, "null-family");
        return format!(
            "INSERT INTO {} ([Marker], [KeyValue], [Id]) VALUES (68, NULL, 4)",
            case.table
        );
    }
    let filter = match case.recipe {
        "minimum" => " WHERE [Id]=1",
        "null" => " WHERE [Id]=2",
        "exterior" => " WHERE [Id]=1 OR [Id]=3",
        "all" => "",
        _ => panic!("outside frozen source"),
    };
    format!("DELETE FROM {}{filter}", case.table)
}

fn readback(case: &Case) -> Vec<u8> {
    let mut text = String::from("Id\tMissing\n");
    for row in sources::rows(case) {
        let Value::Int32(id) = &row[0] else {
            panic!("source Id is manual I32");
        };
        text.push_str(&format!(
            "{id}\t{}\n",
            if row[1] == Value::Null {
                "TRUE"
            } else {
                "FALSE"
            }
        ));
    }
    text.into_bytes()
}

#[test]
#[ignore = "requires 34 native/SDK-qualified numeric source images"]
fn cli_numeric_null_insert_and_signed_nullable_deletes_retire_exact_whole_images() {
    let mut counts = [0; 2];
    for case in sources::load_all(verify) {
        if case.operation == "insert" && case.recipe != "null-family" {
            continue;
        }
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("target.accdb");
        let bundle = dir.path().join("journal");
        fs::write(&path, &case.before).unwrap();
        let statement = sql(&case);
        let command = if case.operation == "insert" {
            "journal-insert"
        } else {
            "journal-delete"
        };
        let output = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args(["query", command, &statement])
            .arg("--database")
            .arg(&path)
            .arg("--journal")
            .arg(&bundle)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        let expected = if case.operation == "insert" {
            "Transaction\tCommitted\nInserted rows\t1\nJournal namespace\tRetired and synchronized\n".to_owned()
        } else {
            let count = sources::ordinals(case.recipe, false).len();
            format!(
                "Transaction\tCommitted\nMatched rows\t{count}\nDeleted rows\t{count}\nJournal namespace\tRetired and synchronized\n"
            )
        };
        assert_eq!(output.stdout, expected.as_bytes());
        assert_eq!(fs::read(&path).unwrap(), case.after);
        assert!(!bundle.exists());
        let query = format!(
            "SELECT [Id], IsNull([KeyValue]) AS Missing FROM {}",
            case.table
        );
        let read = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args(["query", "run", &query])
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
        assert_eq!(read.stdout, readback(&case));
        assert_eq!(fs::read(&path).unwrap(), case.after);
        assert!(!bundle.exists());
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
        counts[usize::from(case.operation == "delete")] += 1;
    }
    assert_eq!(counts, [4, 16]);
}

#[test]
#[ignore = "requires 34 native/SDK-qualified numeric source images"]
fn cli_numeric_primary_refusals_keep_original_bytes_and_create_no_journal() {
    let mut refused = 0;
    for case in sources::load_all(verify)
        .into_iter()
        .filter(|case| case.operation == "insert" && case.recipe == "null-family")
    {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("target.accdb");
        let bundle = dir.path().join("journal");
        fs::write(&path, &case.before).unwrap();
        for (identity, diagnostic) in [
            ("1", "duplicate non-NULL key"),
            ("NULL", "NULL key in primary or required index"),
        ] {
            let statement = format!(
                "INSERT INTO {} ([Id], [KeyValue], [Marker]) VALUES ({identity}, NULL, 68)",
                case.table
            );
            let output = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
                .args(["query", "journal-insert", &statement])
                .arg("--database")
                .arg(&path)
                .arg("--journal")
                .arg(&bundle)
                .output()
                .unwrap();
            assert_eq!(output.status.code(), Some(2));
            assert!(output.stdout.is_empty());
            assert!(String::from_utf8_lossy(&output.stderr).contains(diagnostic));
            assert_eq!(fs::read(&path).unwrap(), case.before);
            assert!(!bundle.exists());
            assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
            refused += 1;
        }
    }
    assert_eq!(refused, 8);
}

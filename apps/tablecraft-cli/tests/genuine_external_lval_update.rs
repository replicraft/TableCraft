#![cfg(any(target_os = "macos", target_os = "linux"))]

use sha2::{Digest, Sha256};
use std::{fs, process::Command};
use tablecraft_core::values::Value;
use tablecraft_testkit::access::genuine_external_lval_update::{self as sources, Case};

fn verify(raw: &[u8], expected: &str) {
    let actual = Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(actual, expected);
}

fn statement(case: &Case) -> Option<String> {
    match case.recipe {
        "memo-last-tail-shrink" => Some(format!(
            "UPDATE Items SET [Memo]='{}' WHERE ID=5",
            "β".repeat(2038)
        )),
        "noop-chain" => Some(format!(
            "UPDATE Items SET [Memo]='{}' WHERE ID=5",
            "Ā".repeat(2039)
        )),
        "empty-group" => Some("UPDATE Items SET [Memo]='unselected' WHERE ID=-1".into()),
        _ => None,
    }
}

#[test]
#[ignore = "requires external32 source qualification and matching normal fitting implementation"]
fn cli_reuses_external_memo_without_invented_binary_literals() {
    let cases = sources::load_all(verify);
    let mut saves = 0;
    for case in cases.iter().filter(|case| statement(case).is_some()) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("target.accdb");
        let bundle = directory.path().join("journal");
        fs::write(&path, &case.before).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args(["query", "journal-update", &statement(case).unwrap()])
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
        let expected = format!(
            "Transaction\tCommitted\nMatched rows\t{}\nChanged rows\t{}\nJournal namespace\tRetired and synchronized\n",
            usize::from(case.recipe != "empty-group"),
            case.changed
        );
        assert_eq!(output.stdout, expected.as_bytes());
        assert_eq!(fs::read(&path).unwrap(), case.after);
        assert!(!bundle.exists());

        // Logical NULL/empty distinctions are queried without exposing or
        // assuming an opaque OLE CLI formatter or coercing Text into Binary.
        let output = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args(["query", "run", "SELECT [ID], IsNull([Memo]) AS MemoMissing, Len([Memo]) AS MemoLength, IsNull([Blob]) AS BlobMissing FROM Items"])
            .arg("--database").arg(&path)
            .output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        let mut expected = String::from("ID\tMemoMissing\tMemoLength\tBlobMissing\n");
        for row in &case.expected {
            let Value::Int32(id) = row[0] else {
                unreachable!()
            };
            let memo_null = matches!(row[3], Value::Null);
            let blob_null = matches!(row[2], Value::Null);
            let length = match &row[3] {
                Value::Null => "NULL".to_owned(),
                Value::Text(value) => value.encode_utf16().count().to_string(),
                _ => unreachable!(),
            };
            expected.push_str(&format!(
                "{id}\t{}\t{length}\t{}\n",
                if memo_null { "TRUE" } else { "FALSE" },
                if blob_null { "TRUE" } else { "FALSE" }
            ));
        }
        assert_eq!(output.stdout, expected.as_bytes());
        assert_eq!(fs::read(&path).unwrap(), case.after);
        assert!(!bundle.exists());
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        saves += 1;
    }
    assert_eq!(saves, 12);
}

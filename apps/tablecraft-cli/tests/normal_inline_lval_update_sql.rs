#![cfg(any(target_os = "macos", target_os = "linux"))]

use sha2::{Digest, Sha256};
use std::{fs, process::Command};
use tablecraft_core::values::Value;
use tablecraft_testkit::fixtures::access::genuine_inline_lval_update::{self as sources, Case};

fn verify(raw: &[u8], expected: &str) {
    let actual = Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(actual, expected);
}

fn statement(case: &Case) -> Option<&'static str> {
    match case.recipe {
        "noop" => Some("UPDATE InlineValues SET MemoValue='old' WHERE Id=1"),
        "memo-new" => Some("UPDATE InlineValues SET MemoValue='new' WHERE Id=1"),
        "raw-unicode" => Some("UPDATE InlineValues SET MemoValue='βγ' WHERE Id=1"),
        "memo-empty" => Some("UPDATE InlineValues SET MemoValue='' WHERE Id=1"),
        "memo-clear" => Some("UPDATE InlineValues SET MemoValue=NULL WHERE Id=1"),
        "blob-clear" => Some("UPDATE InlineValues SET BlobValue=NULL WHERE Id=1"),
        "both-null" => Some("UPDATE InlineValues SET MemoValue=NULL, BlobValue=NULL WHERE Id=1"),
        "empty-group" => Some("UPDATE InlineValues SET MemoValue='old' WHERE Id=-1"),
        _ => None,
    }
}

#[test]
#[ignore = "requires inline28 source qualification and matching normal fitting implementation"]
fn cli_updates_inline_memo_and_null_ole_without_invented_binary_literals() {
    let cases = sources::load_all(verify);
    let mut saves = 0;
    for case in cases.iter().filter(|case| statement(case).is_some()) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("target.accdb");
        let bundle = directory.path().join("journal");
        fs::write(&path, &case.before).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args(["query", "journal-update", statement(case).unwrap()])
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
            "Transaction\tCommitted\n\
                Matched rows\t{}\n\
                Changed rows\t{}\n\
                Journal namespace\tRetired\n\
                Retirement guarantee\tCaptured-parent barrier\n\
                Journal cleanup\tRemoved\n",
            usize::from(case.recipe != "empty-group"),
            case.changed
        );
        assert_eq!(output.stdout, expected.as_bytes());
        assert_eq!(fs::read(&path).unwrap(), case.after);
        assert!(!bundle.exists());

        // Logical NULL/empty distinctions are queried without exposing or
        // assuming an opaque OLE CLI formatter or coercing Text into Binary.
        let output = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args(["query", "run", "SELECT [Id], IsNull([MemoValue]) AS MemoMissing, Len([MemoValue]) AS MemoLength, IsNull([BlobValue]) AS BlobMissing FROM InlineValues"])
            .arg("--database").arg(&path)
            .output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        let memo_null = matches!(case.expected[1], Value::Null);
        let blob_null = matches!(case.expected[2], Value::Null);
        let length = match &case.expected[1] {
            Value::Null => "NULL".to_owned(),
            Value::Text(value) => value.encode_utf16().count().to_string(),
            _ => panic!("only the literal Memo carrier reaches this readback"),
        };
        let expected = format!(
            "Id\tMemoMissing\tMemoLength\tBlobMissing\n1\t{}\t{length}\t{}\n",
            if memo_null { "TRUE" } else { "FALSE" },
            if blob_null { "TRUE" } else { "FALSE" }
        );
        assert_eq!(output.stdout, expected.as_bytes());
        assert_eq!(fs::read(&path).unwrap(), case.after);
        assert!(!bundle.exists());
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        saves += 1;
    }
    assert_eq!(saves, 16);
}

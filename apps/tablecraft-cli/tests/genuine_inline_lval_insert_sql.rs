#![cfg(any(target_os = "macos", target_os = "linux"))]

use sha2::{Digest, Sha256};
use std::{fs, process::Command};
use tablecraft_testkit::fixtures::access::genuine_inline_lval_insert as sources;

fn verify(raw: &[u8], expected: &str) {
    let actual = Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(actual, expected);
}

#[test]
#[ignore = "requires eighteen native/SDK-qualified inline LVAL source views"]
fn cli_inline_memo_empty_and_null_are_distinct_and_retire_exact_files() {
    let mut saves = 0;
    for case in sources::load_all(verify)
        .into_iter()
        .filter(|case| matches!(case.recipe, "null" | "memo-empty-ole-null"))
    {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("target.accdb");
        let bundle = dir.path().join("journal");
        fs::write(&path, &case.before).unwrap();
        let memo = if case.recipe == "null" { "NULL" } else { "''" };
        let sql = format!(
            "INSERT INTO InlineValues ([BlobValue], [MemoValue], [Id]) VALUES (NULL, {memo}, 2)"
        );
        let output = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args(["query", "journal-insert", &sql])
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
        assert_eq!(
            output.stdout,
            concat!(
                "Transaction\tCommitted\n",
                "Inserted rows\t1\n",
                "Journal namespace\tRetired\n",
                "Retirement guarantee\tCaptured-parent barrier\n",
                "Journal cleanup\tRemoved\n",
            )
            .as_bytes()
        );
        assert_eq!(fs::read(&path).unwrap(), case.after);
        assert!(!bundle.exists());
        // Query only logical scalar Id and IsNull(Memo) so this test does not
        // invent a CLI opaque-binary formatter or a binary literal syntax.
        let read = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args([
                "query",
                "run",
                "SELECT [Id], IsNull([MemoValue]) AS Missing FROM InlineValues",
            ])
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
        let expected = if case.recipe == "null" {
            b"Id\tMissing\n1\tFALSE\n2\tTRUE\n".as_slice()
        } else {
            b"Id\tMissing\n1\tFALSE\n2\tFALSE\n".as_slice()
        };
        assert_eq!(read.stdout, expected);
        assert_eq!(fs::read(&path).unwrap(), case.after);
        assert!(!bundle.exists());
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
        saves += 1;
    }
    assert_eq!(saves, 4);
}

#![cfg(any(target_os = "macos", target_os = "linux"))]

use sha2::{Digest, Sha256};
use std::{fs, process::Command};
use tablecraft_core::values::Value;
use tablecraft_testkit::access::genuine_retained_node_insert as sources;

fn verify(raw: &[u8], expected: &str) {
    let hash = Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(hash, expected);
}

#[test]
#[ignore = "requires four source-first NODE INSERT images and native/SDK24 qualification"]
fn cli_node_insert_and_exact_rows_preserve_whole_image_and_duplicate_refusal() {
    for case in sources::load_all(verify) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("target.accdb");
        let bundle = directory.path().join("journal");
        fs::write(&path, &case.before).unwrap();
        let sql = "INSERT INTO Items ([Id],[Score],[Guard]) VALUES (801,400,1)";
        let output = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args(["query", "journal-insert", sql])
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
        assert_eq!(output.stdout, b"Transaction\tCommitted\nInserted rows\t1\nJournal namespace\tRetired and synchronized\n");
        assert_eq!(fs::read(&path).unwrap(), case.after);
        assert!(!bundle.exists());
        let read = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args(["query", "run", "SELECT [Id],[Score],[Guard] FROM Items"])
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
        let mut expected = "Id\tScore\tGuard\n".to_owned();
        for row in sources::rows(true, false) {
            let [Value::Int32(id), Value::Int32(score), Value::Int32(guard)] = row.as_slice()
            else {
                panic!("literal three-Int32 profile");
            };
            expected.push_str(&format!("{id}\t{score}\t{guard}\n"));
        }
        assert_eq!(read.stdout, expected.as_bytes());
        let duplicate = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args(["query", "journal-insert", sql])
            .arg("--database")
            .arg(&path)
            .arg("--journal")
            .arg(&bundle)
            .output()
            .unwrap();
        assert_eq!(duplicate.status.code(), Some(2));
        assert!(duplicate.stdout.is_empty());
        assert!(String::from_utf8_lossy(&duplicate.stderr).contains("duplicate non-NULL key"));
        assert_eq!(fs::read(&path).unwrap(), case.after);
        assert!(!bundle.exists());
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }
}

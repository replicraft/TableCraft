#![cfg(any(target_os = "macos", target_os = "linux"))]

use sha2::{Digest, Sha256};
use std::{fs, process::Command};
use tablecraft_core::values::Value;
use tablecraft_testkit::access::genuine_binary_delete as sources;

fn verify(raw: &[u8], expected: &str) {
    let digest = Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(digest, expected);
}

#[test]
#[ignore = "requires all fourteen independently qualified indexed DELETE source images"]
fn cli_indexed_deletes_use_actual_sql_and_preserve_exact_images_through_empty_follow_on() {
    let mut commits = 0;
    for case in sources::load_all(verify) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("target.accdb");
        let bundle = directory.path().join("journal");
        fs::write(&path, &case.before).unwrap();
        for (sql, removed) in [
            (case.recipe.sql(), case.recipe.selected().len()),
            ("DELETE FROM BinaryKeys WHERE [Id]=-1", 0),
        ] {
            let output = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
                .args(["query", "journal-delete", sql])
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
            assert_eq!(output.stdout, format!(
                "Transaction\tCommitted\nMatched rows\t{removed}\nDeleted rows\t{removed}\nJournal namespace\tRetired and synchronized\n"
            ).as_bytes());
            assert_eq!(fs::read(&path).unwrap(), case.after);
            assert!(!bundle.exists());
            let read = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
                .args([
                    "query",
                    "run",
                    "SELECT [Id], IsNull([KeyValue]) AS Missing, [Marker] FROM BinaryKeys",
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
            let mut expected = "Id\tMissing\tMarker\n".to_owned();
            for row in sources::final_rows(case.recipe) {
                let Value::Int32(id) = row[0] else {
                    panic!("frozen Id carrier");
                };
                let Value::Int32(marker) = row[2] else {
                    panic!("frozen Marker carrier");
                };
                let missing = if row[1] == Value::Null {
                    "TRUE"
                } else {
                    "FALSE"
                };
                expected.push_str(&format!("{id}\t{missing}\t{marker}\n"));
            }
            assert_eq!(read.stdout, expected.as_bytes());
            assert_eq!(fs::read(&path).unwrap(), case.after);
            assert!(!bundle.exists());
            assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
            commits += 1;
        }
    }
    assert_eq!(commits, 24);
}

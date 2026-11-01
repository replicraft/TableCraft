#![cfg(any(target_os = "macos", target_os = "linux"))]

use sha2::{Digest, Sha256};
use std::{fs, process::Command};
use tablecraft_core::values::Value;
use tablecraft_testkit::access::genuine_indexed_delete as sources;

fn verify(raw: &[u8], expected: &str) {
    let digest = Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(digest, expected);
}

#[test]
#[ignore = "requires all sixteen independently qualified indexed DELETE source images"]
fn cli_indexed_deletes_use_actual_sql_and_preserve_exact_images_through_empty_follow_on() {
    let mut commits = 0;
    for case in sources::load_all(verify) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("target.accdb");
        let bundle = directory.path().join("journal");
        fs::write(&path, &case.before).unwrap();
        for (sql, removed) in [
            (case.recipe.sql(), case.recipe.selected().len()),
            ("DELETE FROM Items WHERE [Id]=-1", 0),
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
                .args(["query", "run", "SELECT [Id],Score,Guard FROM Items"])
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
            for row in sources::final_rows(case.recipe) {
                let [Value::Int32(id), Value::Int32(score), Value::Int32(guard)] = row.as_slice()
                else {
                    panic!("closed literal Int32 source rows");
                };
                expected.push_str(&format!("{id}\t{score}\t{guard}\n"));
            }
            assert_eq!(read.stdout, expected.as_bytes());
            assert_eq!(fs::read(&path).unwrap(), case.after);
            assert!(!bundle.exists());
            assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
            commits += 1;
        }
    }
    assert_eq!(commits, 16);
}

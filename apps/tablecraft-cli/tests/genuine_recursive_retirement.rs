#![cfg(any(target_os = "macos", target_os = "linux"))]

use sha2::{Digest, Sha256};
use std::{fs, process::Command};
use tablecraft_core::values::Value;
use tablecraft_testkit::fixtures::access::genuine_recursive_retirement as sources;

fn verify(raw: &[u8], expected: &str) {
    let digest = Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(digest, expected);
}

#[test]
#[ignore = "requires six independent native/SDK-qualified recursive retirement source images"]
fn cli_retained_node_deletes_and_readbacks_preserve_whole_images_through_empty_follow_on() {
    let mut commits = 0;
    for case in sources::load_all(verify) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("target.accdb");
        let bundle = directory.path().join("journal");
        fs::write(&path, &case.before).unwrap();
        for (sql, removed) in [
            (case.recipe.sql(), case.recipe.selected().len()),
            ("DELETE FROM BinaryStorage WHERE [ID]=-1", 0),
        ] {
            let output = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
                .args(["query", "journal-delete", sql])
                // Full source/proof/recovery scans include 149,184 old slots.
                .args(["--work-bytes", "1000000000", "--work-steps", "1000000000"])
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
                .args(["query", "run", "SELECT [ID] FROM BinaryStorage"])
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
            let mut expected = "ID\n".to_owned();
            for row in sources::rows(Some(case.recipe)) {
                let [Value::Int32(id), Value::Binary(bytes)] = row.as_slice() else {
                    panic!("closed literal Int32 source rows");
                };
                assert!(bytes.is_empty());
                expected.push_str(&format!("{id}\n"));
            }
            assert_eq!(read.stdout, expected.as_bytes());
            assert_eq!(fs::read(&path).unwrap(), case.after);
            assert!(!bundle.exists());
            assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
            commits += 1;
        }
    }
    assert_eq!(commits, 8);
}

#![cfg(any(target_os = "macos", target_os = "linux"))]
use sha2::{Digest, Sha256};
use std::{fs, process::Command};
use tablecraft_core::values::Value;
use tablecraft_testkit::fixtures::access::genuine_binary_update as source;

#[test]
#[ignore = "requires 16 qualified fitting Binary UPDATE source views"]
fn cli_binary_indexed_null_updates_retire_exact_images_and_read_all_original_roots() {
    let mut saves = 0;
    for case in source::load_all(|raw, hash| {
        assert_eq!(
            Sha256::digest(raw)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            hash
        )
    }) {
        let Some(sql) = source::sql(case.recipe) else {
            continue;
        };
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("target.accdb");
        let journal = dir.path().join("journal");
        fs::write(&path, &case.before).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args(["query", "journal-update", sql])
            .arg("--database")
            .arg(&path)
            .arg("--journal")
            .arg(&journal)
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
                "Matched rows\t1\n",
                "Changed rows\t1\n",
                "Journal namespace\tRetired\n",
                "Retirement guarantee\tCaptured-parent barrier\n",
                "Journal cleanup\tRemoved\n",
            )
            .as_bytes()
        );
        assert_eq!(fs::read(&path).unwrap(), case.after);
        assert!(!journal.exists());
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
        let mut expected = String::from("Id\tMissing\tMarker\n");
        for row in source::final_rows(case.recipe) {
            let Value::Int32(id) = row[0] else {
                panic!("frozen Id carrier");
            };
            let Value::Int32(marker) = row[2] else {
                panic!("frozen Marker carrier");
            };
            expected.push_str(&format!(
                "{id}\t{}\t{marker}\n",
                if row[1] == Value::Null {
                    "TRUE"
                } else {
                    "FALSE"
                }
            ));
        }
        assert_eq!(read.stdout, expected.as_bytes());
        assert_eq!(fs::read(&path).unwrap(), case.after);
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
        saves += 1;
    }
    assert_eq!(saves, 4);
}

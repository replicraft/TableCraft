#![cfg(any(target_os = "macos", target_os = "linux"))]

use sha2::{Digest, Sha256};
use std::{fs, process::Command};
use tablecraft_core::values::Value;
use tablecraft_testkit::access::genuine_binary_indexed as sources;

fn verify(raw: &[u8], expected: &str) {
    let digest = Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(digest, expected);
}

#[test]
#[ignore = "requires18 independently native/SDK-qualified Binary source views"]
fn cli_null_binary_insert_retires_exact_image_and_reads_nullable_presence() {
    let mut saves = 0;
    for case in sources::load_all(verify)
        .into_iter()
        .filter(|case| case.recipe == "insert-null")
    {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("target.accdb");
        let bundle = dir.path().join("journal");
        fs::write(&path, &case.before).unwrap();

        let output = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args([
                "query",
                "journal-insert",
                "INSERT INTO BinaryKeys ([Marker],[Id],[KeyValue]) VALUES (238,14,NULL)",
            ])
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
            b"Transaction\tCommitted\nInserted rows\t1\nJournal namespace\tRetired and synchronized\n"
        );
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
        let mut expected = String::from("Id\tMissing\tMarker\n");
        for row in case.rows() {
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
        assert!(!bundle.exists());
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
        saves += 1;
    }
    assert_eq!(saves, 2);
}

#[test]
#[ignore = "requires18 independently native/SDK-qualified Binary source views"]
fn cli_binary_primary_refusals_keep_originals_and_create_no_namespace() {
    let mut refusals = 0;
    for case in sources::load_all(verify)
        .into_iter()
        .filter(|case| case.recipe == "insert-null")
    {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("target.accdb");
        let bundle = dir.path().join("journal");
        fs::write(&path, &case.before).unwrap();

        for (id, diagnostic) in [
            ("1", "duplicate non-NULL key"),
            ("NULL", "NULL key in primary or required index"),
        ] {
            let sql =
                format!("INSERT INTO BinaryKeys ([Id],[KeyValue],[Marker]) VALUES ({id},NULL,238)");
            let output = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
                .args(["query", "journal-insert", &sql])
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
            refusals += 1;
        }
    }
    assert_eq!(refusals, 4);
}

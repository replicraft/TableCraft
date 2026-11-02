#![cfg(any(target_os = "macos", target_os = "linux"))]

use sha2::{Digest, Sha256};
use std::{fs, process::Command};
use tablecraft_testkit::fixtures::access::genuine_owned_growth_joint as source;

fn verify(bytes: &[u8], expected: &str) {
    let actual = Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(actual, expected);
}

#[test]
#[ignore = "requires both genuine originals and all four pinned joint AFTERs"]
fn joint_growth_cli_batches_save_and_read_both_assignment_orders() {
    let cases = source::load_all(verify);
    let mut saves = 0;
    for case in cases {
        for reverse in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join("target.accdb");
            let bundle = root.path().join("journal");
            fs::write(&path, &case.before).unwrap();
            let id = case.id.map_or("NULL".into(), |value| value.to_string());
            let assignments = if reverse {
                format!("[ID]={id},Marker='aaaaaaaaaaaaaaaaaaa'")
            } else {
                format!("Marker='aaaaaaaaaaaaaaaaaaa',[ID]={id}")
            };
            let sql = format!("UPDATE Sentinel SET {assignments} WHERE [ID]=23");
            let output = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
                .args(["query", "journal-update", &sql])
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
            assert_eq!(output.stdout, b"Transaction\tCommitted\nMatched rows\t1\nChanged rows\t1\nJournal namespace\tRetired and synchronized\n");
            assert_eq!(fs::read(&path).unwrap(), case.after);
            assert!(!bundle.exists());
            let read = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
                .args(["query", "run", "SELECT [ID],Marker,Bytes FROM Sentinel"])
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
            let expected =
                format!("ID\tMarker\tBytes\n{id}\t\"\"\"aaaaaaaaaaaaaaaaaaa\"\"\"\t0xc700ff\n");
            assert_eq!(read.stdout, expected.as_bytes());
            assert_eq!(fs::read(&path).unwrap(), case.after);
            assert!(!bundle.exists());
            assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
            saves += 1;
        }
    }
    assert_eq!(saves, 8);
}

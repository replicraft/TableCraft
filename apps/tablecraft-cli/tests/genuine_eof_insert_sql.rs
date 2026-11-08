#![cfg(any(target_os = "macos", target_os = "linux"))]

use sha2::{Digest, Sha256};
use std::{fs, process::Command};
use tablecraft_testkit::fixtures::access::genuine_eof_insert as sources;

fn verify(raw: &[u8], expected: &str) {
    let actual = Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(actual, expected);
}

#[test]
#[ignore = "requires both source native/SDK-qualified EOF INSERT directories"]
fn real_cli_complete_single_row_eof_insert_and_exact_full_tsv_match_independent_sources() {
    for case in sources::load_all(verify)
        .into_iter()
        .filter(|case| !case.batch)
    {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("target.accdb");
        let bundle = directory.path().join("journal");
        fs::write(&path, &case.before).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args([
                "query",
                "journal-insert",
                "INSERT INTO Sentinel ([ID],[Marker],[Bytes]) VALUES (260,NULL,NULL)",
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
        let read = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args(["query", "run", "SELECT [ID],[Marker],[Bytes] FROM Sentinel"])
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
        let mut expected =
            String::from("ID\tMarker\tBytes\n23\t\"\"\"unchanged sentinel\"\"\"\t0xc700ff\n");
        for id in 24..=260 {
            expected.push_str(&format!("{id}\tNULL\tNULL\n"));
        }
        assert_eq!(read.stdout, expected.as_bytes());
        assert_eq!(fs::read(&path).unwrap(), case.after);
        assert!(!bundle.exists());
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }
}

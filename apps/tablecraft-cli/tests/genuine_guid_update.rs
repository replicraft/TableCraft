#![cfg(any(target_os = "macos", target_os = "linux"))]
use sha2::{Digest, Sha256};
use std::fs;
use tablecraft_testkit::{
    fixtures::access::genuine_guid_update::{self as source, Recipe},
    support::cli::Runner,
};

const CLI: Runner = Runner::new(env!("CARGO_BIN_EXE_tablecraft"));

#[test]
#[ignore = "requires admitted GUID originals and manual native spans; existing SQL GUID literals"]
fn cli_guid_literals_and_null_save_exact_native_spans_and_read_canonical_values() {
    let cases = source::load_all(|raw, hash| {
        assert_eq!(
            Sha256::digest(raw)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            hash
        );
    });
    let mut saves = 0;
    for case in cases {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("target.accdb");
        let journal = directory.path().join("journal");
        fs::write(&path, &case.before).unwrap();
        let sql = match case.recipe {
            Recipe::Present => {
                "UPDATE Items SET [GuidValue]={GUID {6b29fc40-ca47-1067-b31d-00dd010662da}} WHERE [Int32Value]=17"
            }
            Recipe::NullClear => "UPDATE Items SET [GuidValue]=NULL WHERE [Int32Value]=17",
            Recipe::NullRestore => {
                "UPDATE Items SET [GuidValue]={GUID {00112233-4455-6677-8899-aabbccddeeff}} WHERE [Int32Value]=17"
            }
        };
        let output = CLI.run(&[
            "query",
            "journal-update",
            sql,
            "--database",
            path.to_str().unwrap(),
            "--journal",
            journal.to_str().unwrap(),
        ]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        assert_eq!(output.stdout, b"Transaction\tCommitted\nMatched rows\t1\nChanged rows\t1\nJournal namespace\tRetired and synchronized\n");
        assert_eq!(fs::read(&path).unwrap(), case.after);
        assert!(!journal.exists());

        let output = CLI.run(&[
            "query",
            "run",
            "SELECT [GuidValue] FROM Items WHERE [Int32Value]=17",
            "--database",
            path.to_str().unwrap(),
        ]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        let expected = match case.recipe {
            Recipe::Present => b"GuidValue\n{6b29fc40-ca47-1067-b31d-00dd010662da}\n".as_slice(),
            Recipe::NullClear => b"GuidValue\nNULL\n".as_slice(),
            Recipe::NullRestore => {
                b"GuidValue\n{00112233-4455-6677-8899-aabbccddeeff}\n".as_slice()
            }
        };
        assert_eq!(output.stdout, expected);
        assert_eq!(fs::read(&path).unwrap(), case.after);
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        saves += 1;
    }
    assert_eq!(saves, 6);
}

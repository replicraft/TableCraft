#![cfg(any(target_os = "macos", target_os = "linux"))]
use sha2::{Digest, Sha256};
use std::fs;
use tablecraft_testkit::{
    fixtures::access::genuine_byte_int16_update::{self as source, Recipe},
    support::cli::Runner,
};

const CLI: Runner = Runner::new(env!("CARGO_BIN_EXE_tablecraft"));

#[test]
#[ignore = "requires admitted exact originals and explicit Byte/Int16 span laws"]
fn cli_explicit_small_integral_and_null_assignments_preserve_whole_images() {
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
        let output = CLI.run(&[
            "query",
            "journal-update",
            case.sql(),
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

        let output = CLI.run(&[
            "query",
            "run",
            "SELECT [Int32Value], IsNull([ByteValue]) AS ByteMissing, IsNull([Int16Value]) AS Int16Missing FROM Items WHERE [Int32Value]=17",
            "--database",
            path.to_str().unwrap(),
        ]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        let expected = if case.recipe != Recipe::NullClear {
            b"Int32Value\tByteMissing\tInt16Missing\n17\tFALSE\tFALSE\n".as_slice()
        } else {
            b"Int32Value\tByteMissing\tInt16Missing\n17\tTRUE\tTRUE\n".as_slice()
        };
        assert_eq!(output.stdout, expected);
        assert_eq!(fs::read(&path).unwrap(), case.after);
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        saves += 1;
    }
    assert_eq!(saves, 6);
}

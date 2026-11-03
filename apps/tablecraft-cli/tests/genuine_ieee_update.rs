#![cfg(any(target_os = "macos", target_os = "linux"))]
use sha2::{Digest, Sha256};
use std::fs;
use tablecraft_testkit::{fixtures::access::genuine_ieee_update as source, support::cli::Runner};

const CLI: Runner = Runner::new(env!("CARGO_BIN_EXE_tablecraft"));

#[test]
#[ignore = "requires qualified exact IEEE originals; expected zeros are manual byte variants"]
fn cli_explicit_float_zero_constructors_preserve_negative_zero_date_and_complete_image() {
    let cases = source::load_all(|raw, expected| {
        assert_eq!(
            Sha256::digest(raw)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            expected
        );
    });
    let mut saves = 0;
    for case in cases.into_iter().filter(|case| !case.subnormal_to_special) {
        let start = 8 * 4096 + 3907;
        assert_eq!(&case.before[start + 5..start + 9], &17_i32.to_le_bytes());
        assert_eq!(
            &case.before[start + 25..start + 29],
            &0x8000_0000_u32.to_le_bytes()
        );
        assert_eq!(
            &case.before[start + 29..start + 37],
            &0x8000_0000_0000_0000_u64.to_le_bytes()
        );
        assert_eq!(
            &case.before[start + 37..start + 45],
            &0x8000_0000_0000_0000_u64.to_le_bytes()
        );
        let mut expected = case.before.clone();
        expected[start + 25..start + 37].fill(0);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("target.accdb");
        let journal = directory.path().join("journal");
        fs::write(&path, &case.before).unwrap();
        let output = CLI.run(&[
            "query",
            "journal-update",
            "UPDATE Items SET [SingleValue]=CSng(0),[DoubleValue]=CDbl(0) WHERE [Int32Value]=17",
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
        assert_eq!(fs::read(&path).unwrap(), expected);
        assert!(!journal.exists());

        let output = CLI.run(&[
            "query", "run",
            "SELECT [Int32Value], [SingleValue]=CSng(0) AS SingleZero, [DoubleValue]=CDbl(0) AS DoubleZero FROM Items WHERE [Int32Value]=17",
            "--database", path.to_str().unwrap(),
        ]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        assert_eq!(
            output.stdout,
            b"Int32Value\tSingleZero\tDoubleZero\n17\tTRUE\tTRUE\n"
        );
        assert_eq!(fs::read(&path).unwrap(), expected);
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        saves += 1;
    }
    assert_eq!(saves, 2);
}

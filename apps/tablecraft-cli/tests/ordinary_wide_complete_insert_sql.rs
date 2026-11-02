#![cfg(any(target_os = "macos", target_os = "linux"))]

use std::{fs, process::Command};
use tablecraft_testkit::fixtures::access::canonical_ordinary_wide_insert::{self as sources, Case};

#[test]
fn cli_complete_scalar_null_rows_save_exact_images_and_read_back_supported_finite_columns() {
    let mut saves = 0;
    for case in [Case::FixedNull, Case::DecimalNull] {
        for header in [5_u32, 6] {
            let mut source = sources::source(case);
            source.before[20..24].copy_from_slice(&header.to_le_bytes());
            source.after[20..24].copy_from_slice(&header.to_le_bytes());
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("target.accdb");
            let bundle = directory.path().join("journal");
            fs::write(&path, &source.before).unwrap();
            let (sql, read, expected) = match case {
                Case::FixedNull => (
                    "INSERT INTO Items VALUES (False, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL)",
                    "SELECT [ByteValue], [Int16Value], [Int32Value] FROM Items",
                    "ByteValue\tInt16Value\tInt32Value\n0\t0\t0\n0\t0\t0\nNULL\tNULL\tNULL\n",
                ),
                Case::DecimalNull => (
                    "INSERT INTO Items VALUES (101, NULL, NULL, NULL, False)",
                    "SELECT [Id], [TextValue], [BinaryValue] FROM Items",
                    "Id\tTextValue\tBinaryValue\n1\tNULL\tNULL\n2\tNULL\tNULL\n3\tNULL\tNULL\n101\tNULL\tNULL\n",
                ),
                _ => unreachable!(),
            };
            let output = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
                .args(["query", "journal-insert", sql])
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
            assert_eq!(output.stdout, b"Transaction\tCommitted\nInserted rows\t1\nJournal namespace\tRetired and synchronized\n");
            assert_eq!(fs::read(&path).unwrap(), source.after);
            assert!(!bundle.exists());
            let output = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
                .args(["query", "run", read])
                .arg("--database")
                .arg(&path)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(output.stderr.is_empty());
            assert_eq!(output.stdout, expected.as_bytes());
            assert_eq!(fs::read(&path).unwrap(), source.after);
            assert!(!bundle.exists());
            assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
            saves += 1;
        }
    }
    assert_eq!(saves, 4);
}

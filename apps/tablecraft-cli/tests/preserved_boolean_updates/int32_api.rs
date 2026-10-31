use super::int32_sources::model;
use std::{fs, process::Command};

#[test]
fn preserved_int32_actual_sql_and_typed_commands_match_complete_images_or_refuse() {
    let mut copies = 0;
    let mut refusals = 0;
    for (code, value) in model::CASES {
        let case = model::source(code, value);
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source.accdb");
        fs::write(&source, &case.before).unwrap();
        for typed in [false, true] {
            let output = directory
                .path()
                .join(if typed { "typed.accdb" } else { "sql.accdb" });
            let expression = if matches!(value, i32::MIN | i32::MAX) {
                format!("CLng({value})")
            } else {
                value.to_string()
            };
            let statement = format!("UPDATE Items SET Large_Number={expression} WHERE ID=1");
            let replacement = value.to_string();
            let mut command = Command::new(env!("CARGO_BIN_EXE_tablecraft"));
            if typed {
                command
                    .args(["write", "preview-update"])
                    .arg(&source)
                    .arg(&output)
                    .args([
                        "--set",
                        "Items",
                        "Large_Number",
                        "0",
                        "int32",
                        "1000",
                        &replacement,
                    ]);
            } else {
                command
                    .args(["query", "preview-update", &statement, "--database"])
                    .arg(&source)
                    .arg("--output")
                    .arg(&output);
            }
            let result = command.output().unwrap();
            if code == 5 {
                assert!(!result.status.success());
                assert!(!output.exists());
                refusals += 1;
            } else {
                assert!(
                    result.status.success(),
                    "{}",
                    String::from_utf8_lossy(&result.stderr)
                );
                assert!(fs::read(&output).unwrap() == case.expected);
                copies += 1;
            }
            assert!(fs::read(&source).unwrap() == case.before);
        }
    }
    assert_eq!(copies, 8);
    assert_eq!(refusals, 8);
}

#[test]
fn preserved_int32_joint_copies_and_refusals_preserve_source() {
    let case = model::source(6, 101);
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source.accdb");
    fs::write(&source, &case.before).unwrap();
    for (ordinal, statement) in [
        "UPDATE Items SET [Number]=12 WHERE ID=-1",
        "UPDATE Items SET Large_Number=101,[Number]=12 WHERE ID=1",
        "UPDATE Items SET Large_Number=Calculated WHERE ID=1",
    ]
    .into_iter()
    .enumerate()
    {
        let output = directory.path().join(format!("refused-{ordinal}.accdb"));
        let result = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args(["query", "preview-update", statement, "--database"])
            .arg(&source)
            .arg("--output")
            .arg(&output)
            .output()
            .unwrap();
        assert!(!result.status.success());
        assert!(!output.exists());
        assert!(fs::read(&source).unwrap() == case.before);
    }
    let yes_mask = 13 * 4096 - 1;
    let yes_bit = 1_u8 << 2;
    assert_eq!(case.before[yes_mask] & yes_bit, 0);
    let mut expected = case.expected.clone();
    expected[yes_mask] |= yes_bit;
    for typed in [false, true] {
        let output = directory.path().join(if typed {
            "joint-typed.accdb"
        } else {
            "joint-sql.accdb"
        });
        let mut command = Command::new(env!("CARGO_BIN_EXE_tablecraft"));
        if typed {
            command
                .args(["write", "preview-update"])
                .arg(&source)
                .arg(&output)
                .args([
                    "--set",
                    "Items",
                    "Large_Number",
                    "0",
                    "int32",
                    "1000",
                    "101",
                ])
                .args([
                    "--set",
                    "Items",
                    "Yes_Field",
                    "0",
                    "boolean",
                    "false",
                    "true",
                ]);
        } else {
            command
                .args([
                    "query",
                    "preview-update",
                    "UPDATE Items SET Large_Number=101,Yes_Field=True WHERE ID=1",
                    "--database",
                ])
                .arg(&source)
                .arg("--output")
                .arg(&output);
        }
        let result = command.output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(result.stderr.is_empty());
        assert!(
            String::from_utf8(result.stdout)
                .unwrap()
                .contains("Candidate rows\t1\n")
        );
        assert!(fs::read(&output).unwrap() == expected);
        assert!(fs::read(&source).unwrap() == case.before);
    }
    let stale = directory.path().join("stale.accdb");
    let result = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
        .args(["write", "preview-update"])
        .arg(&source)
        .arg(&stale)
        .args(["--set", "Items", "Large_Number", "0", "int32", "999", "101"])
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(!stale.exists());

    let existing = directory.path().join("existing.accdb");
    fs::write(&existing, b"existing destination").unwrap();
    for output in [&source, &existing] {
        let result = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args([
                "query",
                "preview-update",
                "UPDATE Items SET Large_Number=101 WHERE ID=1",
                "--database",
            ])
            .arg(&source)
            .arg("--output")
            .arg(output)
            .output()
            .unwrap();
        assert!(!result.status.success());
        assert!(fs::read(&source).unwrap() == case.before);
        assert_eq!(fs::read(&existing).unwrap(), b"existing destination");
    }
}

#[test]
fn preserved_int32_null_copy_preserves_inactive_payload_through_both_commands() {
    let case = model::source(6, 101);
    let mut expected = case.before.clone();
    // Row 0 ends at the end of DATA page 12. Its two-byte NULL mask
    // uses bit 4 for the ordinary Int32 target; the payload stays intact.
    expected[13 * 4096 - 2] &= !16;
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source.accdb");
    fs::write(&source, &case.before).unwrap();

    for typed in [false, true] {
        let output = directory.path().join(if typed {
            "typed-null.accdb"
        } else {
            "sql-null.accdb"
        });
        let mut command = Command::new(env!("CARGO_BIN_EXE_tablecraft"));
        if typed {
            command
                .args(["write", "preview-update"])
                .arg(&source)
                .arg(&output)
                .args([
                    "--set",
                    "Items",
                    "Large_Number",
                    "0",
                    "int32",
                    "1000",
                    "null",
                ]);
        } else {
            command
                .args([
                    "query",
                    "preview-update",
                    "UPDATE Items SET Large_Number=NULL WHERE ID=1",
                    "--database",
                ])
                .arg(&source)
                .arg("--output")
                .arg(&output);
        }
        let result = command.output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(fs::read(&output).unwrap(), expected);
        assert_eq!(fs::read(&source).unwrap(), case.before);
    }
}

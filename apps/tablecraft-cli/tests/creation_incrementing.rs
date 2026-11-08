#![cfg(any(target_os = "macos", target_os = "linux"))]
//! Explicit creation selectors followed by the existing omitted INSERT command.
use sha2::{Digest, Sha256};
use std::{fs, process::Command};
use tablecraft_testkit::fixtures::access::genuine_normal_autonumber as fixtures;

#[test]
#[ignore = "requires source14 before matching creator and normal generation"]
fn explicit_cli_declaration_creates_then_generates_one_exact_row_in_both_column_modes() {
    let sources = fixtures::Sources::load(|raw, wanted| {
        assert_eq!(
            Sha256::digest(raw)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            wanted
        );
    });
    for typed in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("created.accdb");
        let bundle = directory.path().join("journal");
        let mut command = Command::new(env!("CARGO_BIN_EXE_tablecraft"));
        command.arg("create").arg(&path).args(["--table", "Items"]);
        if typed {
            command.args(["--field", "Id", "int32", "--field", "Score", "int32"]);
        } else {
            command.args(["--column", "Id", "--column", "Score"]);
        }
        let created = command
            .args(["--primary-key", "Id", "PrimaryId", "--incrementing", "Id"])
            .output()
            .unwrap();
        assert!(
            created.status.success(),
            "{}",
            String::from_utf8_lossy(&created.stderr)
        );
        assert!(created.stderr.is_empty());
        assert_eq!(created.stdout, b"Database\tCreated ACE 2016\nColumns\t2\nRows\t0\nDurable output\tFile and captured-directory barriers completed\n");
        assert_eq!(
            fs::read(&path).unwrap(),
            sources.bytes(5, "increment-zero-before")
        );
        let inserted = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args([
                "query",
                "journal-insert",
                "INSERT INTO Items ([Score]) VALUES (17)",
            ])
            .arg("--database")
            .arg(&path)
            .arg("--journal")
            .arg(&bundle)
            .output()
            .unwrap();
        assert!(
            inserted.status.success(),
            "{}",
            String::from_utf8_lossy(&inserted.stderr)
        );
        assert!(inserted.stderr.is_empty());
        assert_eq!(
            inserted.stdout,
            concat!(
                "Transaction\tCommitted\n",
                "Inserted rows\t1\n",
                "Journal namespace\tRetired\n",
                "Retirement guarantee\tCaptured-parent barrier\n",
                "Journal cleanup\tRemoved\n",
            )
            .as_bytes()
        );
        assert_eq!(
            fs::read(&path).unwrap(),
            sources.bytes(5, "increment-zero-inserted")
        );
        assert!(!bundle.exists());
        let read = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args(["query", "run", "SELECT * FROM Items"])
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
        assert_eq!(read.stdout, b"Id\tScore\n1\t17\n");
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }
}

#[test]
fn invalid_incrementing_selectors_and_nonempty_initial_values_publish_nothing() {
    let cases: &[(&[&str], Option<&str>)] = &[
        (
            &[
                "--field",
                "Id",
                "int32",
                "--field",
                "Score",
                "int32",
                "--incrementing",
                "Id",
            ],
            None,
        ),
        (
            &[
                "--field",
                "Id",
                "int32",
                "--field",
                "Score",
                "int32",
                "--primary-key",
                "Id",
                "PrimaryId",
                "--incrementing",
                "Missing",
            ],
            None,
        ),
        (
            &[
                "--field",
                "Id",
                "int32",
                "--field",
                "Score",
                "int32",
                "--primary-key",
                "Id",
                "PrimaryId",
                "--incrementing",
                "Score",
            ],
            None,
        ),
        (
            &[
                "--field",
                "Id",
                "int64",
                "--field",
                "Score",
                "int32",
                "--primary-key",
                "Id",
                "PrimaryId",
                "--incrementing",
                "Id",
            ],
            None,
        ),
        (
            &[
                "--field",
                "Id",
                "int32",
                "--field",
                "Score",
                "int32",
                "--primary-key",
                "Id",
                "PrimaryId",
                "--incrementing",
                "Id",
                "--incrementing",
                "Id",
            ],
            None,
        ),
        (
            &[
                "--field",
                "Id",
                "int32",
                "--field",
                "Score",
                "int32",
                "--primary-key",
                "Id",
                "PrimaryId",
                "--incrementing",
                "Id",
            ],
            Some("Id\tScore\n23\t17\n"),
        ),
        (
            &[
                "--field",
                "Id",
                "int32",
                "--field",
                "Score",
                "int32",
                "--primary-key",
                "Id",
                "PrimaryId",
                "--incrementing",
                "Id",
            ],
            Some("Id\tScore\nnull\t17\n"),
        ),
    ];
    for (arguments, data) in cases {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("unpublished.accdb");
        let sibling = directory.path().join("sentinel");
        fs::write(&sibling, b"unchanged").unwrap();
        let mut command = Command::new(env!("CARGO_BIN_EXE_tablecraft"));
        command
            .arg("create")
            .arg(&path)
            .args(["--table", "Items"])
            .args(*arguments);
        if let Some(data) = data {
            let input = directory.path().join("input.tsv");
            fs::write(&input, data).unwrap();
            command.arg("--data").arg(input);
        }
        let result = command.output().unwrap();
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        assert!(!result.stderr.is_empty());
        assert!(!path.exists());
        assert_eq!(fs::read(&sibling).unwrap(), b"unchanged");
        assert_eq!(
            fs::read_dir(directory.path()).unwrap().count(),
            if data.is_some() { 2 } else { 1 }
        );
    }
}

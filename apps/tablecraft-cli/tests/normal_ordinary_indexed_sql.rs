#![cfg(any(target_os = "macos", target_os = "linux"))]

use std::{
    fs,
    path::Path,
    process::{Command, Output},
};
use tablecraft_core::values::Value;
use tablecraft_testkit::access::canonical_ordinary_indexed::{self as model, Case, Source};

const PAGE: usize = 4096;

fn update(path: &Path, journal: &Path, sql: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tablecraft"))
        .args(["query", "journal-update", sql])
        .arg("--database")
        .arg(path)
        .arg("--journal")
        .arg(journal)
        .output()
        .unwrap()
}

fn literal_expected(source: &Source) -> (Vec<u8>, String, String) {
    assert_eq!(source.rows[0][0], Value::Int32(1));
    assert_eq!(
        source
            .rows
            .iter()
            .filter(|row| row[0] == Value::Int32(1))
            .count(),
        1
    );
    let base = 8 * PAGE;
    assert_eq!(&source.bytes[base + 4..base + 8], &7_u32.to_le_bytes());
    let word = u16::from_le_bytes(source.bytes[base + 14..base + 16].try_into().unwrap());
    assert_eq!(word & 0xe000, 0);
    let start = base + usize::from(word);
    assert_eq!(&source.bytes[start + 2..start + 6], &1_i32.to_le_bytes());
    let mut expected = source.bytes.clone();
    let (replacement, readback) = if source.target == "Score" {
        assert_eq!(source.columns, ["Id", "Score"]);
        assert_eq!(&source.bytes[start..start + 2], &[2, 0]);
        let Value::Int32(original) = source.rows[0][1] else {
            panic!("the independent Score target has an exact Int32 carrier");
        };
        assert_eq!(
            &source.bytes[start + 6..start + 10],
            &original.to_le_bytes()
        );
        expected[start + 6..start + 10].copy_from_slice(&17_i32.to_le_bytes());
        ("17", "Score\n17\n")
    } else {
        assert_eq!(source.target, "Flag");
        assert_eq!(source.target_position, 4);
        assert_eq!(&source.bytes[start..start + 2], &[5, 0]);
        assert_eq!(source.rows[0][4], Value::Boolean(true));
        // Row zero ends at the DATA page boundary. Boolean stores its value in
        // native bit4 of the one-byte mask, with no fixed payload to overwrite.
        let mask = 9 * PAGE - 1;
        assert_ne!(source.bytes[mask] & 0x10, 0);
        expected[mask] &= !0x10;
        ("False", "Flag\nFALSE\n")
    };
    let sql = format!(
        "UPDATE [Items] SET [{}]={replacement} WHERE [Id]=1",
        source.target
    );
    (expected, sql, readback.to_owned())
}

#[test]
fn retained_index_cli_commits_eighteen_literal_images_and_reads_targets() {
    let mut saves = 0;
    for header in [5_u8, 6] {
        for case in model::CASES {
            let mut source = model::source(case);
            if !source.ordinary {
                continue;
            }
            source.bytes[20..24].copy_from_slice(&u32::from(header).to_le_bytes());
            // Complete image and readback expectations precede all children.
            // No index/counter metadata is rewritten by this literal overlay.
            let (expected, sql, readback) = literal_expected(&source);
            let select = format!("SELECT [{}] FROM [Items] WHERE [Id]=1", source.target);
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join("target.accdb");
            let journal = root.path().join("journal");
            fs::write(&path, &source.bytes).unwrap();

            let output = update(&path, &journal, &sql);
            assert!(
                output.status.success(),
                "{}: {}",
                case.id(),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(output.stderr.is_empty());
            assert_eq!(output.stdout, b"Transaction\tCommitted\nMatched rows\t1\nChanged rows\t1\nJournal namespace\tRetired and synchronized\n");
            assert_eq!(fs::read(&path).unwrap(), expected);
            assert!(!journal.exists());
            assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);

            let rows = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
                .args(["query", "run", &select])
                .arg("--database")
                .arg(&path)
                .args(["--format", "tsv"])
                .output()
                .unwrap();
            assert!(
                rows.status.success(),
                "{}: {}",
                case.id(),
                String::from_utf8_lossy(&rows.stderr)
            );
            assert!(rows.stderr.is_empty());
            assert_eq!(rows.stdout, readback.as_bytes());
            assert_eq!(fs::read(&path).unwrap(), expected);
            assert!(!journal.exists());
            saves += 1;
        }
    }
    assert_eq!(saves, 18);
}

#[test]
fn generated_and_indexed_id_requests_refuse_even_when_no_row_matches() {
    for header in [5_u8, 6] {
        for case in [Case::CounterOnly, Case::CounterPrimary, Case::UniqueManual] {
            let mut source = model::source(case);
            source.bytes[20..24].copy_from_slice(&u32::from(header).to_le_bytes());
            assert!(source.ordinary);
            for predicate in ["[Id]=1", "[Id]=-1"] {
                let root = tempfile::tempdir().unwrap();
                let path = root.path().join("target.accdb");
                let journal = root.path().join("journal");
                fs::write(&path, &source.bytes).unwrap();
                let sql = format!("UPDATE [Items] SET [Id]=7 WHERE {predicate}");
                let output = update(&path, &journal, &sql);
                assert!(!output.status.success(), "{}: {predicate}", case.id());
                assert!(output.stdout.is_empty());
                assert!(!output.stderr.is_empty());
                assert_eq!(fs::read(&path).unwrap(), source.bytes);
                assert!(!journal.exists());
                assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
            }
        }
    }
}

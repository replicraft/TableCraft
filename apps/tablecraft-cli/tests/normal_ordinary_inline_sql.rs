#![cfg(any(target_os = "macos", target_os = "linux"))]

use std::{
    fmt::Write as _,
    fs,
    path::Path,
    process::{Command, Output},
};
use tablecraft_testkit::fixtures::access::canonical_ordinary_inline::{self as model, Case};

fn update(path: &Path, journal: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tablecraft"))
        .args([
            "query",
            "journal-update",
            "UPDATE Items SET Score=17 WHERE Id=1",
        ])
        .arg("--database")
        .arg(path)
        .arg("--journal")
        .arg(journal)
        .output()
        .unwrap()
}

#[test]
fn dynamic_inline_cli_commits_ten_literal_images_and_reads_exact_rows() {
    let mut saves = 0;
    for header in [5, 6] {
        for case in model::CASES {
            let mut source = model::source(case);
            if !source.ordinary {
                continue;
            }
            source.bytes[20] = header;
            assert_eq!(source.bytes.len(), 96 * 4096);
            assert_eq!(source.data, 95);
            assert_eq!(source.rows[0][0], 1);
            assert_eq!(source.rows.iter().filter(|row| row[0] == 1).count(), 1);

            // Both expected outputs are complete before invoking the CLI. The
            // only file edit is row zero's independently bounded Score payload.
            let page = 95 * 4096;
            let word = u16::from_le_bytes(source.bytes[page + 14..page + 16].try_into().unwrap());
            assert_eq!(word & 0xe000, 0);
            let start = page + usize::from(word);
            assert_eq!(&source.bytes[start..start + 2], &[2, 0]);
            assert_eq!(&source.bytes[start + 2..start + 6], &1_i32.to_le_bytes());
            assert_eq!(
                &source.bytes[start + 6..start + 10],
                &source.rows[0][1].to_le_bytes()
            );
            assert_eq!(source.bytes[start + 10], 3);
            let mut expected = source.bytes.clone();
            expected[start + 6..start + 10].copy_from_slice(&17_i32.to_le_bytes());
            let mut expected_rows = String::from("Id\tScore\n");
            for (ordinal, row) in source.rows.iter().enumerate() {
                let score = if ordinal == 0 { 17 } else { row[1] };
                writeln!(expected_rows, "{}\t{score}", row[0]).unwrap();
            }

            let root = tempfile::tempdir().unwrap();
            let path = root.path().join("target.accdb");
            let journal = root.path().join("journal");
            fs::write(&path, &source.bytes).unwrap();
            let output = update(&path, &journal);
            assert!(
                output.status.success(),
                "{}: {}",
                case.id(),
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
            assert_eq!(fs::read(&path).unwrap(), expected);
            assert!(!journal.exists());
            assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);

            let rows = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
                .arg("rows")
                .arg(&path)
                .args(["Items", "--format", "tsv"])
                .output()
                .unwrap();
            assert!(
                rows.status.success(),
                "{}",
                String::from_utf8_lossy(&rows.stderr)
            );
            assert!(rows.stderr.is_empty());
            assert_eq!(rows.stdout, expected_rows.as_bytes());
            assert_eq!(fs::read(&path).unwrap(), expected);
            assert!(!journal.exists());
            saves += 1;
        }
    }
    assert_eq!(saves, 10);
}

#[test]
fn dynamic_inline_unknown_bounds_and_indirect_maps_refuse_before_publication() {
    for header in [5, 6] {
        for case in [
            Case::OwnedPastEof,
            Case::CandidatePastEof,
            Case::MissingGlobalCoverage,
            Case::Indirect,
        ] {
            let mut source = model::source(case);
            source.bytes[20] = header;
            assert!(!source.ordinary);
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join("target.accdb");
            let journal = root.path().join("journal");
            fs::write(&path, &source.bytes).unwrap();

            let output = update(&path, &journal);
            assert!(!output.status.success(), "{}", case.id());
            assert!(output.stdout.is_empty());
            assert!(!output.stderr.is_empty());
            assert_eq!(fs::read(&path).unwrap(), source.bytes);
            assert!(!journal.exists());
            assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
        }
    }
}

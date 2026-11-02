#![cfg(any(target_os = "macos", target_os = "linux"))]

use std::{
    fs,
    path::Path,
    process::{Command, Output},
};
use tablecraft_core::values::Value;
use tablecraft_testkit::fixtures::access::canonical_ordinary_content::{self as model, Case};

fn run(path: &Path, journal: &Path, sql: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tablecraft"))
        .args(["query", "journal-update", sql])
        .arg("--database")
        .arg(path)
        .arg("--journal")
        .arg(journal)
        .output()
        .unwrap()
}

fn first_record(bytes: &[u8], page: u32) -> usize {
    let base = page as usize * 4096;
    let word = u16::from_le_bytes(bytes[base + 14..base + 16].try_into().unwrap());
    assert_eq!(word & 0xe000, 0);
    base + usize::from(word)
}

fn committed(before: &[u8], expected: &[u8], sql: &str, matched: usize, changed: usize) {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("target.accdb");
    let journal = root.path().join("journal");
    fs::write(&path, before).unwrap();

    let output = run(&path, &journal, sql);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let report = format!(
        "Transaction\tCommitted\nMatched rows\t{matched}\nChanged rows\t{changed}\nJournal namespace\tRetired and synchronized\n"
    );
    assert_eq!(output.stdout, report.as_bytes());
    assert_eq!(fs::read(&path).unwrap(), expected);
    assert!(!journal.exists());
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
}

#[test]
fn ordinary_cli_commits_twenty_two_literal_complete_images() {
    let mut saves = 0;
    for header in [5, 6] {
        for case in [
            Case::Extrema,
            Case::Named,
            Case::ProgramLikeName,
            Case::XmlData,
            Case::Multiple,
            Case::ReorderedCatalog,
            Case::RelocatedUser,
            Case::RelocatedMaps,
            Case::SplitData,
            Case::TrailingFreePage,
        ] {
            let mut source = model::source(case);
            source.bytes[20] = header;
            let table = source
                .tables
                .iter()
                .find(|table| !table.rows.is_empty())
                .unwrap();
            let Value::Int32(before) = table.rows[0][0] else {
                panic!("the independent selected first column must be Int32");
            };
            assert_eq!(
                table
                    .rows
                    .iter()
                    .filter(|row| row[0] == Value::Int32(before))
                    .count(),
                1
            );
            let mut expected = source.bytes.clone();
            let start = first_record(&source.bytes, table.data[0]);
            assert_eq!(&expected[start + 2..start + 6], &before.to_le_bytes());
            expected[start + 2..start + 6].copy_from_slice(&73_i32.to_le_bytes());
            let sql = format!(
                "UPDATE [{}] SET [{}]=73 WHERE [{}]={before}",
                table.name, table.columns[0], table.columns[0],
            );
            committed(&source.bytes, &expected, &sql, 1, 1);
            saves += 1;
        }

        let mut source = model::source(Case::InactiveXml);
        source.bytes[20] = header;
        let mut expected = source.bytes.clone();
        let start = first_record(&source.bytes, 8);
        assert_eq!(&expected[start + 9..start + 17], b"<Event/>");
        assert_eq!(&expected[start + 17..start + 25], &1_i64.to_le_bytes());
        assert_eq!(&expected[start + 37..start + 45], &0_u64.to_le_bytes());
        assert_eq!(expected[start + 61] & 0x11, 0);
        expected[start + 9..start + 17].copy_from_slice(&7_i64.to_le_bytes());
        expected[start + 17..start + 25].copy_from_slice(&(-10001_i64).to_le_bytes());
        expected[start + 37..start + 45].copy_from_slice(&0x3ff4_0000_0000_0000_u64.to_le_bytes());
        expected[start + 61] |= 0x11;
        // Only row zero has absent Int64; both original Flag values are false.
        assert_eq!(source.tables[0].rows[0][4], Value::Null);
        assert_eq!(source.tables[0].rows[1][4], Value::Int64(0));
        let sql = "UPDATE Items SET Flag=True,Int64Value=CLngLng(7),[Money]=CCur(-1.0001),DateValue=#1899-12-31 06:00:00# WHERE Int64Value Is Null";
        committed(&source.bytes, &expected, sql, 1, 1);
        saves += 1;
    }
    assert_eq!(saves, 22);
}

#[test]
fn ordinary_equal_and_empty_selections_commit_without_changing_any_bytes() {
    for header in [5, 6] {
        let mut source = model::source(Case::Extrema);
        source.bytes[20] = header;
        let table = &source.tables[0];
        let Value::Int32(before) = table.rows[0][0] else {
            panic!("ordinary original first value must be Int32");
        };
        let equal = format!("UPDATE Items SET Id={before} WHERE Id={before}");
        committed(&source.bytes, &source.bytes, &equal, 1, 0);
        committed(
            &source.bytes,
            &source.bytes,
            "UPDATE Items SET Id=73 WHERE Id=-1234567",
            0,
            0,
        );
    }
}

#[test]
fn unknown_content_refuses_without_target_or_journal_changes() {
    for case in [
        Case::OpaqueExtra,
        Case::UnknownHeaderByte,
        Case::OrphanPayloadPage,
    ] {
        let source = model::source(case);
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("target.accdb");
        let journal = root.path().join("journal");
        fs::write(&path, &source.bytes).unwrap();

        let output = run(&path, &journal, "UPDATE Items SET Id=73 WHERE Id=1");
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
        assert_eq!(fs::read(&path).unwrap(), source.bytes);
        assert!(!journal.exists());
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    }
}

#[test]
fn created_database_supports_normal_journal_update_and_rows_readback() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("created.accdb");
    let data = root.path().join("rows.tsv");
    let journal = root.path().join("journal");
    fs::write(&data, b"Id\tScore\n1\t10\n2\t20\n").unwrap();

    let created = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
        .arg("create")
        .arg(&path)
        .args([
            "--table", "Items", "--column", "Id", "--column", "Score", "--data",
        ])
        .arg(&data)
        .output()
        .unwrap();
    assert!(
        created.status.success(),
        "{}",
        String::from_utf8_lossy(&created.stderr)
    );
    assert!(created.stderr.is_empty());
    assert_eq!(created.stdout, b"Database\tCreated ACE 2016\nColumns\t2\nRows\t2\nDurable output\tFile and captured-directory barriers completed\n");

    // Capture the actual created file before UPDATE. The independent expectation
    // changes only row zero's literal Score payload, after checking both rows'
    // complete direct-record geometry and original values.
    let before = fs::read(&path).unwrap();
    assert_eq!(before.len(), 9 * 4096);
    let page = 8 * 4096;
    assert_eq!(&before[page + 12..page + 18], &[2, 0, 245, 15, 234, 15]);
    assert_eq!(
        &before[page + 4085..page + 4096],
        &[2, 0, 1, 0, 0, 0, 10, 0, 0, 0, 3]
    );
    assert_eq!(
        &before[page + 4074..page + 4085],
        &[2, 0, 2, 0, 0, 0, 20, 0, 0, 0, 3]
    );
    let mut expected = before.clone();
    expected[page + 4091..page + 4095].copy_from_slice(&12_i32.to_le_bytes());
    let frozen_before = root.path().join("before.accdb");
    let frozen_after = root.path().join("expected.accdb");
    fs::write(&frozen_before, &before).unwrap();
    fs::write(&frozen_after, &expected).unwrap();

    let updated = run(&path, &journal, "UPDATE Items SET Score=12 WHERE Id=1");
    assert!(
        updated.status.success(),
        "{}",
        String::from_utf8_lossy(&updated.stderr)
    );
    assert!(updated.stderr.is_empty());
    assert_eq!(updated.stdout, b"Transaction\tCommitted\nMatched rows\t1\nChanged rows\t1\nJournal namespace\tRetired and synchronized\n");
    assert_eq!(fs::read(&path).unwrap(), expected);
    assert!(!journal.exists());

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
    assert_eq!(rows.stdout, b"Id\tScore\n1\t12\n2\t20\n");
    assert_eq!(fs::read(&path).unwrap(), expected);
    assert_eq!(fs::read(&frozen_before).unwrap(), before);
    assert_eq!(fs::read(&frozen_after).unwrap(), expected);
    assert_eq!(fs::read(&data).unwrap(), b"Id\tScore\n1\t10\n2\t20\n");
}

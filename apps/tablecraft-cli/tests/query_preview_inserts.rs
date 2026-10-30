//! Original-source literal framing is fixed before SQL INSERT candidate calls.
#[path = "query_preview_inserts/default_clock.rs"]
mod default_clock;
#[path = "query_preview_inserts/display_metadata.rs"]
mod display_metadata;
#[path = "query_preview_inserts/omissions.rs"]
mod omissions;
#[path = "query_preview_inserts/typed_omissions.rs"]
mod typed_omissions;

#[path = "../../../crates/engines/tablecraft-ace/tests/mutation/fixture.rs"]
mod fixture;
#[path = "../../../crates/engines/tablecraft-ace/tests/mutation/index_fixture.rs"]
#[allow(dead_code)]
mod index_fixture;
#[path = "write_previews/model.rs"]
#[allow(dead_code)]
mod model;
use std::{
    fs,
    io::Cursor,
    process::{Command, Output},
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access as support;

const PAGE: usize = 4096;

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

fn source(code: u8, generated: bool) -> Vec<u8> {
    let mut bytes = model::fixed(generated);
    bytes[20] = code;
    bytes
}

fn expected(before: &[u8], generated: bool) -> Vec<u8> {
    let mut after = before.to_vec();
    let score = if generated { 31_i32 } else { 40_i32 };
    // These three source rows are fixed11B with3 live slots. A fourth record
    // fits the same DATA12 and the existing4B common key prefix remains valid.
    assert_eq!(&before[12 * PAGE + 12..12 * PAGE + 14], &[3, 0]);
    assert_eq!(&before[13 * PAGE + 24..13 * PAGE + 26], &[4, 0]);
    assert_eq!(&before[13 * PAGE + 480..13 * PAGE + 484], &[127, 128, 0, 0]);
    let record = [
        2_u16.to_le_bytes().as_slice(),
        &4_i32.to_le_bytes(),
        &score.to_le_bytes(),
        &[3],
    ]
    .concat();
    assert_eq!(record.len(), 11);
    let end = usize::from(u16::from_le_bytes(
        before[12 * PAGE + 18..12 * PAGE + 20].try_into().unwrap(),
    ));
    let start = end - record.len();
    after[12 * PAGE + start..12 * PAGE + end].copy_from_slice(&record);
    support::put16(&mut after, 12 * PAGE + 20, start as u16);
    support::put16(&mut after, 12 * PAGE + 12, 4);
    support::put16(&mut after, 12 * PAGE + 2, (start - 22) as u16);
    support::put32(&mut after, 10 * PAGE + 16, 4);
    support::put32(&mut after, 10 * PAGE + 67, 4);
    if generated {
        support::put32(&mut after, 10 * PAGE + 20, 31);
    }
    let leaf = &mut after[13 * PAGE..14 * PAGE];
    leaf[27..480].fill(0);
    for end in [9_usize, 14, 19, 24] {
        leaf[27 + end / 8] |= 1 << (end % 8);
    }
    leaf[499..504].copy_from_slice(&[score as u8, 0, 0, 12, 3]);
    support::put16(leaf, 2, 3592);
    after
}

fn checked(bytes: &[u8], count: usize) {
    let report = tablecraft_ace::check_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(report.rows(), count);
    assert!(
        report
            .indexes()
            .iter()
            .all(|index| index.status() == tablecraft_ace::IndexCheckStatus::Checked)
    );
    let constraints = tablecraft_ace::check_table_constraints(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(constraints.rows(), count);
    assert!(!constraints.fields().iter().any(|field| {
        [field.required(), field.empty_text(), field.validation()]
            .contains(&tablecraft_ace::RuleCheckStatus::Unsupported)
    }));
}

#[test]
fn qualify_sql_insert_literal_sources_and_whole_expected_images_before_preview() {
    for code in [5, 6] {
        for generated in [false, true] {
            let before = source(code, generated);
            let after = expected(&before, generated);
            checked(&before, 3);
            checked(&after, 4);
            let rows = tablecraft_ace::read_table(
                &mut Cursor::new(&after),
                CatalogScope::new(1),
                "Items",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                rows.rows()[3],
                [
                    Value::Int32(4),
                    Value::Int32(if generated { 31 } else { 40 })
                ]
            );
            assert_eq!(after.len(), before.len());
            for page in [1, 4, 11, 14] {
                assert_eq!(
                    &after[page * PAGE..(page + 1) * PAGE],
                    &before[page * PAGE..(page + 1) * PAGE]
                );
            }
        }
    }
}

fn run(sql: &str, input: &std::path::Path, output: &std::path::Path, incrementing: bool) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_tablecraft"));
    command.args([
        "query",
        "preview-insert",
        sql,
        "--database",
        input.to_str().unwrap(),
        "--output",
        output.to_str().unwrap(),
    ]);
    if incrementing {
        command.arg("--incrementing");
    }
    command.output().unwrap()
}

#[test]
fn sql_values_manual_and_explicit_incrementing_match_frozen_whole_images() {
    for code in [5, 6] {
        for generated in [false, true] {
            let before = source(code, generated);
            let wanted = expected(&before, generated);
            let directory = tempfile::tempdir().unwrap();
            let input = directory.path().join("source.accdb");
            let output = directory.path().join("candidate.accdb");
            fs::write(&input, &before).unwrap();
            let sql = if generated {
                "INSERT INTO Items (Id) VALUES (2+2)"
            } else {
                "INSERT INTO Items (Score,Id) VALUES (40,4)"
            };
            let result = run(sql, &input, &output, generated);
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert!(result.stderr.is_empty());
            let report = String::from_utf8(result.stdout).unwrap();
            assert!(report.contains("Experimental"));
            assert!(report.contains("Candidate rows\t1\n"));
            if generated {
                assert!(report.contains("Generated column\t1\n"));
                assert!(report.contains("Generated value[0]\t31\n"));
            }
            let actual = fs::read(&output).unwrap();
            assert_eq!(actual.len(), wanted.len());
            assert_eq!(
                actual
                    .iter()
                    .zip(&wanted)
                    .position(|(actual, wanted)| actual != wanted),
                None
            );
            assert_eq!(fs::read(&input).unwrap(), before);
        }
    }
}

#[test]
fn unsupported_shapes_and_missing_companion_lists_refuse_before_database_open() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("missing.accdb");
    let output = directory.path().join("unpublished.accdb");
    for (sql, incrementing) in [
        ("INSERT INTO Items VALUES (4)", true),
        ("INSERT INTO Items SELECT Id,Score FROM Other", false),
        ("INSERT INTO Items VALUES (UserFunction(),4)", false),
        ("INSERT INTO Items VALUES (Default,4)", false),
    ] {
        let result = run(sql, &input, &output, incrementing);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        assert!(!output.exists());
        assert!(!String::from_utf8_lossy(&result.stderr).contains("NotFound"));
    }
}

//! Actual SQL multi-column commands over the source-first nullable tuple owner.
use tablecraft_testkit::access::mutation_fixture as fixture;
#[path = "../../../crates/backends/tablecraft-access/src/engineering/editor/tests/batch/model.rs"]
mod model;
use tablecraft_testkit::access as support;

use std::{fs, io::Cursor, path::Path, process::Command};
use tablecraft_ace::read_table;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

fn assert_rows(bytes: &[u8], expected: &[[Option<i32>; 2]; 3]) {
    let rows = read_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000),
        &|| false,
    )
    .unwrap()
    .into_parts()
    .2;
    for (ordinal, row) in rows.iter().enumerate() {
        assert_eq!(
            row,
            &vec![
                Value::Int32(ordinal as i32 + 1),
                expected[ordinal][0].map_or(Value::Null, Value::Int32),
                expected[ordinal][1].map_or(Value::Null, Value::Int32)
            ]
        );
    }
}

fn run(sql: &str, input: &Path, output: &Path, flags: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_tablecraft"))
        .args(["query", "preview-update", sql, "--database"])
        .arg(input)
        .arg("--output")
        .arg(output)
        .args(flags)
        .output()
        .unwrap()
}

#[test]
fn complete_original_row_assignments_publish_the_frozen_tuple_images() {
    let directory = tempfile::tempdir().unwrap();
    for code in [5, 6] {
        for (name, recipe, sql, matched, changed) in [
            (
                "clear",
                model::Recipe::Clear,
                "UPDATE Items SET [Group]=NULL, Score=NULL WHERE Id=1",
                1,
                1,
            ),
            (
                "exchange",
                model::Recipe::Exchange,
                "UPDATE Items SET [Group]=Score, Score=[Group]",
                3,
                2,
            ),
            (
                "introduce",
                model::Recipe::Introduce,
                "UPDATE Items SET [Group]=20, Score=200 WHERE Id=3",
                1,
                1,
            ),
        ] {
            let example = model::case(code, recipe);
            let input = directory.path().join(format!("{code}-{name}-source.accdb"));
            let output = directory
                .path()
                .join(format!("{code}-{name}-candidate.accdb"));
            fs::write(&input, &example.before).unwrap();
            assert_rows(&example.before, &example.original);
            let result = run(sql, &input, &output, &[]);
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert!(result.stderr.is_empty());
            let report = String::from_utf8(result.stdout).unwrap();
            assert!(report.contains(&format!("Matched rows\t{matched}\n")));
            assert!(report.contains(&format!("Candidate rows\t{changed}\n")));
            let after = fs::read(&output).unwrap();
            assert_eq!(after, example.expected);
            assert_rows(&after, &example.final_values);
            assert_eq!(fs::read(&input).unwrap(), example.before);
        }
    }
}

#[test]
fn duplicate_targets_and_late_second_expression_errors_publish_nothing() {
    let directory = tempfile::tempdir().unwrap();
    let example = model::case(5, model::Recipe::Exchange);
    let input = directory.path().join("original.accdb");
    fs::write(&input, &example.before).unwrap();
    for (position, sql) in [
        "UPDATE Items AS i SET i.Score=1, score=2 WHERE FALSE",
        "UPDATE Items SET [Group]=Id, Score=2147483645+Id",
        "UPDATE Items SET [Group]=Id, Score=1 WHERE FALSE AND 1/0=0",
    ]
    .iter()
    .enumerate()
    {
        let output = directory.path().join(format!("refused-{position}.accdb"));
        let result = run(sql, &input, &output, &[]);
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
        assert!(!output.exists());
        assert_eq!(fs::read(&input).unwrap(), example.before);
    }
}

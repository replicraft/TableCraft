//! Real SQL UPDATE consumes the exact engine-owned historical literal model.
use tablecraft_testkit::access::mutation_fixture as base;
#[path = "../../../crates/engines/tablecraft-ace/src/table/mutation/fixed/historical_columns/model.rs"]
mod model;

use std::{fs, io::Cursor, path::Path, process::Command};
use tablecraft_ace::{
    IndexCheckStatus, RuleCheckStatus, check_table, check_table_constraints, read_table,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

fn qualify(bytes: &[u8], expected: &[Vec<Value>]) {
    let table = read_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(table.rows(), expected);

    let checked = check_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(checked.rows(), 3);
    assert_eq!(checked.indexes().len(), 2);
    assert!(
        checked
            .indexes()
            .iter()
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );

    let report = check_table_constraints(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_ne!(report.table_validation(), RuleCheckStatus::Unsupported);
    assert!(report.fields().iter().all(|field| {
        field.required() != RuleCheckStatus::Unsupported
            && field.empty_text() != RuleCheckStatus::Unsupported
            && field.validation() != RuleCheckStatus::Unsupported
    }));
    assert!(report.relationships().is_empty());
}

fn update(input: &Path, output: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_tablecraft"))
        .args([
            "query",
            "preview-update",
            "UPDATE Items SET Score=Id*8+9 WHERE Id<=2",
            "--database",
        ])
        .arg(input)
        .arg("--output")
        .arg(output)
        .output()
        .unwrap()
}

#[test]
fn qualify_historical_cli_sources_and_prefrozen_expected_before_commands() {
    for code in [5, 6] {
        for variable in [false, true] {
            for selected_number in [2, 4] {
                let case = model::case(code, selected_number == 4, variable, true, selected_number);
                qualify(&case.before, &case.rows);
                qualify(&case.expected, &case.final_rows);
                assert_eq!(case.variable, variable);
                assert_eq!(case.selected_number, selected_number);
            }
        }
    }
}

#[test]
fn sql_updates_native_identity_gaps_and_matches_frozen_whole_images() {
    for code in [5, 6] {
        for variable in [false, true] {
            for selected_number in [2, 4] {
                let case = model::case(code, selected_number == 4, variable, true, selected_number);
                qualify(&case.before, &case.rows);
                qualify(&case.expected, &case.final_rows);
                let directory = tempfile::tempdir().unwrap();
                let input = directory.path().join("source.accdb");
                let output = directory.path().join("updated.accdb");
                fs::write(&input, &case.before).unwrap();

                let result = update(&input, &output);
                assert!(
                    result.status.success(),
                    "{}",
                    String::from_utf8_lossy(&result.stderr)
                );
                assert!(result.stderr.is_empty());
                let report = String::from_utf8(result.stdout).unwrap();
                assert!(report.contains("Matched rows\t2\n"));
                assert!(report.contains("Candidate rows\t2\n"));
                assert_eq!(fs::read(&output).unwrap(), case.expected);
                assert_eq!(fs::read(&input).unwrap(), case.before);
            }
        }
    }
}

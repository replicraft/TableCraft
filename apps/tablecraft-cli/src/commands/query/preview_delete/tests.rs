use super::{Arguments, run};
use crate::commands::rows::stream::tests::fixture;
use clap::Parser;
use std::{fs, io::Cursor, path::Path};
use tablecraft_ace::{IndexCheckStatus, check_table, check_table_constraints};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

#[derive(Parser)]
struct Invocation {
    #[command(flatten)]
    args: Arguments,
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

fn source() -> Vec<u8> {
    let mut before = fixture::database();
    before[4..20].copy_from_slice(b"Standard ACE DB\0");
    before[20] = 5;
    before
}

fn expected(before: &[u8]) -> Vec<u8> {
    let mut after = before.to_vec();
    after[10 * 4096 + 16..10 * 4096 + 20].copy_from_slice(&2_u32.to_le_bytes());
    let slot = 12 * 4096 + 16;
    let word = u16::from_le_bytes(before[slot..slot + 2].try_into().unwrap());
    after[slot..slot + 2].copy_from_slice(&(word | 0xc000).to_le_bytes());
    after
}

fn qualify(bytes: &[u8], rows: usize) {
    let checked = check_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(checked.rows(), rows);
    assert!(
        checked
            .indexes()
            .iter()
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );
    let constraints = check_table_constraints(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(constraints.relationships().is_empty());
}

fn args(sql: &str, input: &Path, output: &Path) -> Arguments {
    Invocation::try_parse_from([
        "test",
        sql,
        "--database",
        input.to_str().unwrap(),
        "--output",
        output.to_str().unwrap(),
    ])
    .unwrap()
    .args
}

#[test]
fn qualify_cli_sql_delete_source_and_complete_expected_before_handler() {
    let before = source();
    qualify(&before, 3);
    qualify(&expected(&before), 2);
}

#[test]
fn delete_handler_publishes_literal_complete_copy_and_preserves_source() {
    let before = source();
    let expected = expected(&before);
    qualify(&before, 3);
    qualify(&expected, 2);
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("source.accdb");
    let output = directory.path().join("copy.accdb");
    fs::write(&input, &before).unwrap();
    let report = run(args("DELETE FROM Items WHERE Id=2", &input, &output), None)
        .unwrap_or_else(|error| panic!("DELETE handler failed: {error}"));
    assert!(report.contains("Matched rows\t1\n"));
    assert!(report.contains("Deleted rows\t1\n"));
    assert_eq!(fs::read(&output).unwrap(), expected);
    assert_eq!(fs::read(&input).unwrap(), before);
    assert!(run(args("DELETE FROM Items WHERE Id=2", &input, &output), None).is_err());
    assert_eq!(fs::read(&output).unwrap(), expected);
}

#[test]
fn syntax_and_late_predicate_errors_leave_no_output_or_temporary() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("source.accdb");
    let output = directory.path().join("copy.accdb");
    let before = source();
    qualify(&before, 3);
    for sql in ["DELETE wrong.* FROM Items", "UPDATE Items SET Score=1"] {
        assert!(run(args(sql, &input, &output), None).is_err());
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 0);
    }
    fs::write(&input, &before).unwrap();
    assert!(
        run(
            args("DELETE FROM Items WHERE 10/(2-Id)>0", &input, &output),
            None
        )
        .is_err()
    );
    assert!(!output.exists());
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    assert_eq!(fs::read(&input).unwrap(), before);
}

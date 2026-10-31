//! Multiple tables retain the existing required fresh-file publication protocol.
use super::budget;
use crate::creation::{CreationDestination, CreationError};
use std::fs;
use tablecraft_ace::{CreateColumn, CreateDatabaseError, CreateTable};
use tablecraft_core::values::Value;
use tablecraft_schema::{DecimalConstraints, ScalarType};
use tablecraft_testkit::access::canonical_multiple_creation as source;

fn column(column: &source::Column) -> CreateColumn<'_> {
    match column.native_type {
        4 => CreateColumn::new(&column.name, ScalarType::Int32),
        1 => CreateColumn::new(&column.name, ScalarType::Boolean),
        10 => CreateColumn::short_text(&column.name, usize::from(column.width) / 2).unwrap(),
        9 => CreateColumn::short_binary(&column.name, usize::from(column.width)).unwrap(),
        16 => {
            let (precision, scale) = column.decimal.unwrap();
            CreateColumn::decimal(
                &column.name,
                DecimalConstraints::new(precision, scale).unwrap(),
            )
        }
        _ => panic!("unexpected independent multiple-table SOURCE declaration"),
    }
}

#[test]
fn multiple_table_service_publishes_all_four_independent_complete_images() {
    for case in source::CASES {
        let source = source::source(case);
        let columns = source
            .tables
            .iter()
            .map(|table| table.columns.iter().map(column).collect::<Vec<_>>())
            .collect::<Vec<_>>();
        let rows = source
            .tables
            .iter()
            .map(|table| table.rows.iter().map(Vec::as_slice).collect::<Vec<_>>())
            .collect::<Vec<_>>();
        let tables = source
            .tables
            .iter()
            .enumerate()
            .map(|(index, table)| CreateTable::new(&table.name, &columns[index], &rows[index]))
            .collect::<Vec<_>>();
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("multiple.accdb");
        let mut allowance = budget();
        let destination = CreationDestination::capture(&path, &mut allowance, &|| false).unwrap();
        let prepared = destination
            .prepare_tables(&tables, &mut allowance, &|| false)
            .unwrap();
        let expected_rows = source
            .tables
            .iter()
            .map(|table| table.rows.len())
            .sum::<usize>();
        let expected_columns = source
            .tables
            .iter()
            .map(|table| table.columns.len())
            .sum::<usize>();
        assert_eq!(
            (
                prepared.table_count(),
                prepared.row_count(),
                prepared.column_count()
            ),
            (source.tables.len(), expected_rows, expected_columns)
        );
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);

        let receipt = prepared.publish(&mut allowance, &|| false).unwrap();
        assert!(receipt.namespace_synced());
        assert_eq!(
            (
                receipt.table_count(),
                receipt.row_count(),
                receipt.column_count()
            ),
            (source.tables.len(), expected_rows, expected_columns)
        );
        assert_eq!(fs::read(&path).unwrap(), source.bytes, "case {}", case.id());
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    }
}

#[test]
fn invalid_late_table_creates_no_staging_or_final_entry() {
    let columns = [CreateColumn::new("Id", ScalarType::Int32)];
    let good = [Value::Int32(1)];
    let wrong = [Value::Int64(2)];
    let first = [good.as_slice()];
    let last = [wrong.as_slice()];
    let tables = [
        CreateTable::new("First", &columns, &first),
        CreateTable::new("Last", &columns, &last),
    ];
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("multiple.accdb");
    let mut allowance = budget();
    let destination = CreationDestination::capture(&path, &mut allowance, &|| false).unwrap();
    let error = destination
        .prepare_tables(&tables, &mut allowance, &|| false)
        .err()
        .unwrap();
    assert!(matches!(
        error,
        CreationError::Create(CreateDatabaseError::Carrier)
    ));
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
}

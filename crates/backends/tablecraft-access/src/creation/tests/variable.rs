//! Typed variable declarations use the existing required creation publisher.
use super::budget;
use crate::creation::CreationDestination;
use std::fs;
use tablecraft_ace::CreateColumn;
use tablecraft_schema::{DecimalConstraints, ScalarType};
use tablecraft_testkit::fixtures::access::canonical_native17_creation as native17;
use tablecraft_testkit::fixtures::access::canonical_variable_creation;

#[test]
fn typed_service_publishes_five_complete_variable_source_images() {
    for case in canonical_variable_creation::CASES {
        let mut source = canonical_variable_creation::source(case);
        source.bytes = native17::source(source.bytes);
        let columns = source
            .columns
            .iter()
            .map(|column| match column.native_type {
                4 => CreateColumn::new(column.name, ScalarType::Int32),
                10 => CreateColumn::short_text(column.name, usize::from(column.width) / 2).unwrap(),
                9 => CreateColumn::short_binary(column.name, usize::from(column.width)).unwrap(),
                16 => CreateColumn::decimal(
                    column.name,
                    DecimalConstraints::new(column.precision, column.scale).unwrap(),
                ),
                1 => CreateColumn::new(column.name, ScalarType::Boolean),
                _ => panic!("unexpected independent variable SOURCE declaration"),
            })
            .collect::<Vec<_>>();
        let rows = source.rows.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("variable.accdb");
        let mut allowance = budget();
        let destination = CreationDestination::capture(&path, &mut allowance, &|| false).unwrap();
        let prepared = destination
            .prepare_typed(source.table, &columns, &rows, &mut allowance, &|| false)
            .unwrap();

        assert_eq!(
            (prepared.row_count(), prepared.column_count()),
            (source.rows.len(), source.columns.len())
        );
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
        let receipt = prepared.publish(&mut allowance, &|| false).unwrap();
        super::assert_publication(&receipt);
        assert_eq!(
            (receipt.row_count(), receipt.column_count()),
            (source.rows.len(), source.columns.len())
        );
        assert_eq!(fs::read(&path).unwrap(), source.bytes, "case {}", case.id());
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    }
}

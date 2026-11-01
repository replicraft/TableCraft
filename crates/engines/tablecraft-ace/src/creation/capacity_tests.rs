//! Whole-image candidate-map boundary checks after independent SOURCE qualification.
use super::{CreateColumn, create_ace16_database};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_schema::{DecimalConstraints, ScalarType};
use tablecraft_testkit::access::canonical_native17_creation as native17;
use tablecraft_testkit::access::canonical_variable_capacity_creation;

#[test]
fn variable_creation_matches_both_independent_capacity_boundary_images() {
    for case in canonical_variable_capacity_creation::CASES {
        let mut original = canonical_variable_capacity_creation::source(case);
        original.source.bytes = native17::source(original.source.bytes);
        let source = &original.source;
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
                _ => panic!("unexpected independent capacity SOURCE declaration"),
            })
            .collect::<Vec<_>>();
        let rows = source.rows.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let mut budget = ResourceBudget::new(16 * 1024 * 1024, 100_000, 16_000_000);
        let created =
            create_ace16_database(source.table, &columns, &rows, &mut budget, &|| false).unwrap();

        assert_eq!(created.as_bytes(), source.bytes, "case {}", case.id());
        assert_eq!(created.row_count(), source.rows.len());
        assert_eq!(created.column_count(), source.columns.len());
    }
}

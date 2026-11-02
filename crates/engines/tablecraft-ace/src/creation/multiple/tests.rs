//! Independent whole images and refusal controls for one multi-table artifact.
use super::super::{
    CreateColumn, CreateDatabaseError, CreateTable, create_ace16_database,
    create_ace16_database_tables,
};
use crate::InspectError;
use std::cell::Cell;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_schema::{DecimalConstraints, ScalarType};
use tablecraft_testkit::fixtures::access::canonical_native17_creation as native17;
use tablecraft_testkit::fixtures::access::{
    canonical_creation as fixed, canonical_multiple_creation as multiple,
    canonical_variable_creation as variable,
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

fn column(column: &multiple::Column) -> CreateColumn<'_> {
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
fn four_multiple_table_original_images_match_all_explicit_roles_and_aggregate_counts() {
    for case in multiple::CASES {
        let mut source = multiple::source(case);
        source.bytes = native17::source(source.bytes);
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
        let created = create_ace16_database_tables(&tables, &mut budget(), &|| false).unwrap();

        assert_eq!(created.as_bytes(), source.bytes, "case {}", case.id());
        assert_eq!(created.table_count(), source.tables.len());
        assert_eq!(
            created.row_count(),
            source
                .tables
                .iter()
                .map(|table| table.rows.len())
                .sum::<usize>()
        );
        assert_eq!(
            created.column_count(),
            source
                .tables
                .iter()
                .map(|table| table.columns.len())
                .sum::<usize>()
        );
    }
}

fn compare_one(table: &str, columns: &[CreateColumn<'_>], rows: &[&[Value]], expected: &[u8]) {
    let old = create_ace16_database(table, columns, rows, &mut budget(), &|| false).unwrap();
    let created = create_ace16_database_tables(
        &[CreateTable::new(table, columns, rows)],
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(old.as_bytes(), expected);
    assert_eq!(created.as_bytes(), expected);
    assert_eq!(old.table_count(), 1);
    assert_eq!(created.table_count(), 1);
    assert_eq!(
        (old.row_count(), old.column_count()),
        (created.row_count(), created.column_count())
    );
}

#[test]
fn one_table_multiple_operation_preserves_existing_fixed_and_variable_source_images() {
    for case in fixed::CASES {
        let mut source = fixed::source(case);
        source.bytes = native17::source(source.bytes);
        let columns = source
            .columns
            .iter()
            .map(|name| CreateColumn::new(name, ScalarType::Int32))
            .collect::<Vec<_>>();
        let values = source
            .rows
            .iter()
            .map(|row| row.iter().copied().map(Value::Int32).collect::<Vec<_>>())
            .collect::<Vec<_>>();
        let rows = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
        compare_one(source.table, &columns, &rows, &source.bytes);
    }
    for case in variable::CASES {
        let mut source = variable::source(case);
        source.bytes = native17::source(source.bytes);
        let columns = [
            CreateColumn::new("Id", ScalarType::Int32),
            CreateColumn::short_text("TextValue", 8).unwrap(),
            CreateColumn::short_binary("BinaryValue", 16).unwrap(),
            CreateColumn::decimal("DecimalValue", DecimalConstraints::new(28, 4).unwrap()),
            CreateColumn::new("Flag", ScalarType::Boolean),
        ];
        let rows = source.rows.iter().map(Vec::as_slice).collect::<Vec<_>>();
        compare_one(source.table, &columns, &rows, &source.bytes);
    }
}

#[test]
fn table_count_and_folded_duplicate_names_refuse_without_an_artifact() {
    let columns = [CreateColumn::new("Id", ScalarType::Int32)];
    assert!(matches!(
        create_ace16_database_tables(&[], &mut budget(), &|| false),
        Err(CreateDatabaseError::TableLimit)
    ));
    let too_many = [CreateTable::new("Same", &columns, &[]); 17];
    assert!(matches!(
        create_ace16_database_tables(&too_many, &mut budget(), &|| false),
        Err(CreateDatabaseError::TableLimit)
    ));
    let duplicates = [
        CreateTable::new("Items", &columns, &[]),
        CreateTable::new("ITEMS", &columns, &[]),
    ];
    assert!(matches!(
        create_ace16_database_tables(&duplicates, &mut budget(), &|| false),
        Err(CreateDatabaseError::DuplicateTable)
    ));
    let system = [
        CreateTable::new("First", &columns, &[]),
        CreateTable::new("msysobjects", &columns, &[]),
    ];
    assert!(matches!(
        create_ace16_database_tables(&system, &mut budget(), &|| false),
        Err(CreateDatabaseError::SystemTableName)
    ));
}

#[test]
fn late_table_carrier_shape_and_data_overflow_refuse_the_complete_operation() {
    let columns = [CreateColumn::new("Id", ScalarType::Int32)];
    let valid = [Value::Int32(7)];
    let wrong = [Value::Int64(7)];
    let empty: [Value; 0] = [];
    for (late, expected_carrier) in [(wrong.as_slice(), true), (empty.as_slice(), false)] {
        let first = [valid.as_slice()];
        let last = [late];
        let tables = [
            CreateTable::new("First", &columns, &first),
            CreateTable::new("Last", &columns, &last),
        ];
        let error = create_ace16_database_tables(&tables, &mut budget(), &|| false)
            .err()
            .unwrap();
        assert!(matches!(
            (error, expected_carrier),
            (CreateDatabaseError::Carrier, true) | (CreateDatabaseError::RowWidth, false)
        ));
    }
    let names = (0..16).map(|index| format!("C{index}")).collect::<Vec<_>>();
    let wide = names
        .iter()
        .map(|name| CreateColumn::new(name, ScalarType::Int32))
        .collect::<Vec<_>>();
    let row = vec![Value::Int32(1); 16];
    let rows = vec![row.as_slice(); 64];
    let tables = [
        CreateTable::new("First", &columns, &[]),
        CreateTable::new("Wide", &wide, &rows),
    ];
    assert!(matches!(
        create_ace16_database_tables(&tables, &mut budget(), &|| false),
        Err(CreateDatabaseError::PageCapacity)
    ));
}

#[test]
fn multiple_table_work_is_cumulative_and_final_cancellation_discards_the_image() {
    let columns = [CreateColumn::new("Id", ScalarType::Int32)];
    let row = [Value::Int32(7)];
    let rows = [row.as_slice()];
    let tables = [
        CreateTable::new("First", &columns, &rows),
        CreateTable::new("Second", &columns, &rows),
    ];
    for mut limited in [
        ResourceBudget::new(0, 100_000, 100_000_000),
        ResourceBudget::new(256 * 1024 * 1024, 0, 100_000_000),
        ResourceBudget::new(256 * 1024 * 1024, 100_000, 0),
    ] {
        assert!(matches!(
            create_ace16_database_tables(&tables, &mut limited, &|| false),
            Err(CreateDatabaseError::Inspect(_))
        ));
    }
    assert!(matches!(
        create_ace16_database_tables(&tables, &mut budget(), &|| true),
        Err(CreateDatabaseError::Inspect(InspectError::Cancelled))
    ));
    let polls = Cell::new(0);
    create_ace16_database_tables(&tables, &mut budget(), &|| {
        polls.set(polls.get() + 1);
        false
    })
    .unwrap();
    let final_poll = polls.get();
    polls.set(0);
    assert!(matches!(
        create_ace16_database_tables(&tables, &mut budget(), &|| {
            polls.set(polls.get() + 1);
            polls.get() == final_poll
        }),
        Err(CreateDatabaseError::Inspect(InspectError::Cancelled))
    ));
}

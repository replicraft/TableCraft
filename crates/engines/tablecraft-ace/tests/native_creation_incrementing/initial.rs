//! Empty one-field and ordinary-companion declarations have independent images.
use super::{budget, resources};
use tablecraft_ace::{
    CreateColumn, CreateDatabaseError, CreateTable, create_ace16_database,
    create_ace16_database_tables,
};
use tablecraft_core::values::Value;
use tablecraft_schema::{DecimalConstraints, ScalarType};
use tablecraft_testkit::fixtures::access::canonical_initial_table_creation::{
    self as fixtures, Case,
};

fn declarations(columns: &[fixtures::Column], reverse_builders: bool) -> Vec<CreateColumn<'_>> {
    columns
        .iter()
        .enumerate()
        .map(|(position, column)| {
            let scalar = match column.native_type {
                1 => ScalarType::Boolean,
                2 => ScalarType::Byte,
                3 => ScalarType::Int16,
                4 => ScalarType::Int32,
                5 => ScalarType::Currency,
                6 => ScalarType::Single,
                7 => ScalarType::Double,
                8 => ScalarType::DateTime,
                15 => ScalarType::Guid,
                19 => ScalarType::Int64,
                9 => {
                    return CreateColumn::short_binary(column.name, usize::from(column.width))
                        .unwrap();
                }
                10 => {
                    return CreateColumn::short_text(column.name, usize::from(column.width / 2))
                        .unwrap();
                }
                16 => {
                    let constraints =
                        DecimalConstraints::new(column.precision, column.scale).unwrap();
                    return CreateColumn::decimal(column.name, constraints);
                }
                _ => panic!("closed independent native declaration"),
            };
            let field = CreateColumn::new(column.name, scalar);
            if position != 0 {
                return field;
            }
            if reverse_builders {
                field
                    .with_incrementing_autonumber()
                    .unwrap()
                    .with_primary_key("PrimaryId")
                    .unwrap()
            } else {
                field
                    .with_primary_key("PrimaryId")
                    .unwrap()
                    .with_incrementing_autonumber()
                    .unwrap()
            }
        })
        .collect()
}

#[test]
fn id_only_text_and_mixed_companions_match_independent_complete_images() {
    for case in fixtures::CASES {
        let expected = fixtures::source(case);
        for reverse_builders in [false, true] {
            let columns = declarations(expected.columns, reverse_builders);
            let rows: [&[Value]; 0] = [];
            for multiple in [false, true] {
                let created = if multiple {
                    create_ace16_database_tables(
                        &[CreateTable::new("Items", &columns, &rows)],
                        &mut budget(),
                        &|| false,
                    )
                } else {
                    create_ace16_database("Items", &columns, &rows, &mut budget(), &|| false)
                }
                .unwrap();
                assert_eq!(created.as_bytes(), expected.bytes, "{case:?}");
                assert_eq!(created.table_count(), 1);
                assert_eq!(created.column_count(), columns.len());
                assert_eq!(created.row_count(), 0);
            }
        }
    }
}

#[test]
fn generalized_empty_increment_keeps_generation_primary_and_policy_restrictions() {
    let expected = fixtures::source(Case::ShortText);
    let fields = declarations(expected.columns, false);
    let rows: [&[Value]; 0] = [];
    let wrong_order = [fields[1], fields[0]];
    let duplicate = [fields[0], fields[0]];
    let policy = [fields[0], fields[1].with_required(false)];
    let default = Value::Text("name".into());
    let with_default = [fields[0], fields[1].with_default_literal(&default).unwrap()];
    assert!(create_ace16_database("Items", &with_default, &rows, &mut budget(), &|| false).is_ok());
    for columns in [&wrong_order, &duplicate, &policy] {
        assert!(create_ace16_database("Items", columns, &rows, &mut budget(), &|| false).is_err());
    }
    let row = [Value::Null, Value::Text("name".into())];
    assert!(matches!(
        create_ace16_database("Items", &fields, &[&row], &mut budget(), &|| false),
        Err(CreateDatabaseError::Declaration)
    ));
}

#[test]
fn mixed_empty_increment_pays_exact_resources_and_observes_cancellation() {
    let expected = fixtures::source(Case::Mixed);
    let columns = declarations(expected.columns, false);
    let rows: [&[Value]; 0] = [];
    for multiple in [false, true] {
        resources::assert_resources(&expected.bytes, |work, cancelled| {
            if multiple {
                create_ace16_database_tables(
                    &[CreateTable::new("Items", &columns, &rows)],
                    work,
                    cancelled,
                )
            } else {
                create_ace16_database("Items", &columns, &rows, work, cancelled)
            }
        });
    }
}

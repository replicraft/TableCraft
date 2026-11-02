//! Matching creator checks consume already qualified independent SOURCE only.
use super::{
    CreateColumn, CreateDatabaseError, CreateTable, create_ace16_database,
    create_ace16_database_tables,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_schema::{DecimalConstraints, FieldConstraintViolation, ScalarType};
use tablecraft_testkit::fixtures::access::canonical_creation_properties::{
    self as sources, Violation,
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn columns(source: &sources::Source) -> Vec<CreateColumn<'static>> {
    source
        .columns
        .iter()
        .map(|field| {
            let mut column = match field.native_type {
                4 => CreateColumn::new(field.name, ScalarType::Int32),
                10 => CreateColumn::short_text(field.name, usize::from(field.width) / 2).unwrap(),
                9 => CreateColumn::short_binary(field.name, usize::from(field.width)).unwrap(),
                16 => CreateColumn::decimal(
                    field.name,
                    DecimalConstraints::new(field.precision, field.scale).unwrap(),
                ),
                1 => CreateColumn::new(field.name, ScalarType::Boolean),
                _ => panic!("source declaration outside the frozen family"),
            };
            if let Some(policy) = source
                .policies
                .iter()
                .find(|policy| policy.column == field.name)
            {
                if let Some(required) = policy.required {
                    column = column.with_required(required);
                }
                if let Some(allow) = policy.allow_zero_length {
                    column = column.with_allow_zero_length(allow).unwrap();
                }
            }
            column
        })
        .collect()
}

#[test]
fn typed_creation_boolean_policies_keep_absence_false_and_declared_type_distinct() {
    let column = CreateColumn::short_text("Text", 20).unwrap();
    assert_eq!(column.required(), None);
    assert_eq!(column.allow_zero_length(), None);
    let column = column
        .with_required(false)
        .with_allow_zero_length(true)
        .unwrap();
    assert_eq!(column.required(), Some(false));
    assert_eq!(column.allow_zero_length(), Some(true));
    let column = column
        .with_required(true)
        .with_allow_zero_length(false)
        .unwrap();
    assert_eq!(column.required(), Some(true));
    assert_eq!(column.allow_zero_length(), Some(false));
    assert!(matches!(
        CreateColumn::short_binary("Bytes", 16)
            .unwrap()
            .with_allow_zero_length(true),
        Err(CreateDatabaseError::Declaration)
    ));
    assert!(matches!(
        CreateColumn::new("Number", ScalarType::Int32).with_allow_zero_length(false),
        Err(CreateDatabaseError::Declaration)
    ));
}

#[test]
fn both_creation_builders_match_all_seven_literal_property_sources_and_refuse_bad_rows() {
    let mut outputs = 0;
    let mut refused = 0;
    for case in sources::CASES {
        let source = sources::source(case);
        let columns = columns(&source);
        let rows = source.rows.iter().map(Vec::as_slice).collect::<Vec<_>>();
        for multiple in [false, true] {
            let result = if multiple {
                create_ace16_database_tables(
                    &[CreateTable::new("Items", &columns, &rows)],
                    &mut budget(),
                    &|| false,
                )
            } else {
                create_ace16_database("Items", &columns, &rows, &mut budget(), &|| false)
            };
            if let Some(violation) = source.violation {
                let expected = match violation {
                    Violation::Required => FieldConstraintViolation::Required,
                    Violation::EmptyText => FieldConstraintViolation::EmptyText,
                };
                assert!(
                    matches!(result, Err(CreateDatabaseError::FieldConstraint(actual)) if actual == expected)
                );
                refused += 1;
            } else {
                let created = result.unwrap();
                assert_eq!(created.as_bytes(), source.bytes);
                assert_eq!(created.row_count(), source.rows.len());
                assert_eq!(created.column_count(), 5);
                assert_eq!(created.table_count(), 1);
                outputs += 1;
            }
        }
        if let Some(violation) = source.violation {
            let mut no_image_bytes = ResourceBudget::new(0, 1_000_000, 1_000_000);
            let result =
                create_ace16_database("Items", &columns, &rows, &mut no_image_bytes, &|| false);
            let expected = match violation {
                Violation::Required => FieldConstraintViolation::Required,
                Violation::EmptyText => FieldConstraintViolation::EmptyText,
            };
            assert!(
                matches!(result, Err(CreateDatabaseError::FieldConstraint(actual)) if actual == expected)
            );
        }
    }
    assert_eq!((outputs, refused), (14, 4));
}

#[test]
fn complete_multi_table_catalog_property_size_refuses_before_image_allocation() {
    let names = (0..16)
        .map(|index| format!("Field{index:02}{}", "x".repeat(57)))
        .collect::<Vec<_>>();
    assert!(names.iter().all(|name| name.encode_utf16().count() == 64));
    let columns = names
        .iter()
        .map(|name| {
            CreateColumn::short_text(name, 255)
                .unwrap()
                .with_required(false)
                .with_allow_zero_length(true)
                .unwrap()
        })
        .collect::<Vec<_>>();
    let rows: [&[Value]; 0] = [];
    for name in ["First", "Second"] {
        super::request::validate(
            name,
            super::input::Columns::Values(&columns),
            super::input::Rows::Values(&rows),
            &mut budget(),
            &|| false,
        )
        .unwrap();
    }
    // Each complete declaration fits in isolation. The combined catalog does
    // not; no catalog/property page allocator is silently substituted.
    let result = create_ace16_database_tables(
        &[
            CreateTable::new("First", &columns, &rows),
            CreateTable::new("Second", &columns, &rows),
        ],
        &mut budget(),
        &|| false,
    );
    assert!(matches!(result, Err(CreateDatabaseError::PageCapacity)));
}

#[path = "property_tests/additional.rs"]
mod additional;

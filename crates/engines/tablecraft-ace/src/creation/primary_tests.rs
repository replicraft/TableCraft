//! Matching manual-primary creator acceptance uses independently frozen sources.
use super::{
    CreateColumn, CreateDatabaseError, CreateTable, create_ace16_database,
    create_ace16_database_tables,
};
use crate::IndexConstraintViolation;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_schema::ScalarType;
use tablecraft_testkit::access::canonical_creation_primary::{self as sources, Violation};

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

#[test]
fn primary_request_is_one_explicit_manual_int32_identifier_without_page_authority() {
    let ordinary = CreateColumn::new("Id", ScalarType::Int32);
    assert_eq!(ordinary.primary_key_name(), None);
    let primary = ordinary.with_primary_key("PrimaryId").unwrap();
    assert_eq!(primary.primary_key_name(), Some("PrimaryId"));
    assert_eq!(primary.required(), None);
    assert_eq!(primary.default_literal(), None);
    for scalar in [
        ScalarType::Boolean,
        ScalarType::Byte,
        ScalarType::Int16,
        ScalarType::Int64,
        ScalarType::Currency,
        ScalarType::Single,
        ScalarType::Double,
        ScalarType::Guid,
        ScalarType::Text,
        ScalarType::Binary,
        ScalarType::Decimal,
        ScalarType::DateTime,
    ] {
        assert!(matches!(
            CreateColumn::new("Other", scalar).with_primary_key("PrimaryId"),
            Err(CreateDatabaseError::Declaration)
        ));
    }
    let rows: [&[Value]; 0] = [];
    let two = [
        primary,
        CreateColumn::new("Score", ScalarType::Int32)
            .with_primary_key("Another")
            .unwrap(),
    ];
    assert!(matches!(
        create_ace16_database("Items", &two, &rows, &mut budget(), &|| false),
        Err(CreateDatabaseError::Declaration)
    ));
    let bad = [ordinary.with_primary_key("Bad.Name").unwrap()];
    assert!(matches!(
        create_ace16_database("Items", &bad, &rows, &mut budget(), &|| false),
        Err(CreateDatabaseError::InvalidName)
    ));
}

#[test]
fn both_creators_match_four_complete_primary_sources_and_refuse_the_two_constraint_inputs() {
    let mut outputs = 0;
    let mut refused = 0;
    let columns = [
        CreateColumn::new("Id", ScalarType::Int32)
            .with_primary_key("PrimaryId")
            .unwrap(),
        CreateColumn::new("Score", ScalarType::Int32),
    ];
    for case in sources::CASES {
        let source = sources::source(case);
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
                    Violation::DuplicateKey => IndexConstraintViolation::DuplicateKey,
                    Violation::NullKey => IndexConstraintViolation::NullKey,
                };
                assert!(
                    matches!(result, Err(CreateDatabaseError::IndexConstraint(actual)) if actual == expected)
                );
                refused += 1;
            } else {
                let created = result.unwrap();
                assert_eq!(
                    created.as_bytes(),
                    source.bytes.as_slice(),
                    "{} multiple={multiple}",
                    case.id()
                );
                assert_eq!(created.table_count(), 1);
                assert_eq!(created.column_count(), 2);
                assert_eq!(created.row_count(), source.rows.len());
                outputs += 1;
            }
        }
        if let Some(violation) = source.violation {
            let expected = match violation {
                Violation::DuplicateKey => IndexConstraintViolation::DuplicateKey,
                Violation::NullKey => IndexConstraintViolation::NullKey,
            };
            let mut no_image = ResourceBudget::new(0, 1_000_000, 1_000_000);
            assert!(
                matches!(create_ace16_database("Items", &columns, &rows, &mut no_image, &|| false),
                             Err(CreateDatabaseError::IndexConstraint(actual)) if actual == expected)
            );
        }
    }
    assert_eq!((outputs, refused), (8, 4));
}

#[path = "primary_tests/resources.rs"]
mod resources;

#[path = "primary_tests/components.rs"]
mod components;

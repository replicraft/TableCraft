//! Source-backed manual wide primary creation; no generated key semantics.
#[path = "integral/resources.rs"]
mod resources;

use super::{
    CreateColumn, CreateDatabaseError, CreateTable, budget, create_ace16_database,
    create_ace16_database_tables,
};
use crate::{CreatedDatabase, IndexConstraintViolation};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_schema::ScalarType;
use tablecraft_testkit::access::canonical_creation_integral_primary::{self as sources, Family};

fn columns(family: Family) -> [CreateColumn<'static>; 2] {
    let scalar = match family {
        Family::Int64 => ScalarType::Int64,
        Family::Currency => ScalarType::Currency,
    };
    [
        CreateColumn::new("Id", scalar)
            .with_primary_key("PrimaryId")
            .unwrap(),
        CreateColumn::new("Score", ScalarType::Int32),
    ]
}

fn invoke(
    source: &sources::Source,
    multiple: bool,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<CreatedDatabase, CreateDatabaseError> {
    let columns = columns(source.family);
    let rows = source.rows.iter().map(Vec::as_slice).collect::<Vec<_>>();
    if multiple {
        create_ace16_database_tables(
            &[CreateTable::new("Items", &columns, &rows)],
            work,
            cancelled,
        )
    } else {
        create_ace16_database("Items", &columns, &rows, work, cancelled)
    }
}

#[test]
fn both_creators_match_literal_wide_primaries_and_refuse_duplicate_or_null_keys_before_image() {
    let mut positive = 0;
    let mut negative = 0;
    for case in sources::CASES {
        let source = sources::source(case);
        for multiple in [false, true] {
            let result = invoke(&source, multiple, &mut budget(), &|| false);
            if let Some(violation) = source.violation {
                let expected = match violation {
                    sources::Violation::DuplicateKey => IndexConstraintViolation::DuplicateKey,
                    sources::Violation::NullKey => IndexConstraintViolation::NullKey,
                };
                assert!(matches!(
                    result,
                    Err(CreateDatabaseError::IndexConstraint(actual)) if actual == expected
                ));
                negative += 1;
                continue;
            }
            let created = result.unwrap();
            assert_eq!(
                created.as_bytes(),
                source.bytes,
                "{} multiple={multiple}",
                case.id()
            );
            assert_eq!(created.table_count(), 1);
            assert_eq!(created.column_count(), 2);
            assert_eq!(created.row_count(), source.rows.len());
            positive += 1;
        }
        if let Some(violation) = source.violation {
            let expected = match violation {
                sources::Violation::DuplicateKey => IndexConstraintViolation::DuplicateKey,
                sources::Violation::NullKey => IndexConstraintViolation::NullKey,
            };
            let mut no_image = ResourceBudget::new(0, 1_000_000, 1_000_000_000);
            assert!(matches!(
                invoke(&source, false, &mut no_image, &|| false),
                Err(CreateDatabaseError::IndexConstraint(actual)) if actual == expected
            ));
            negative += 1;
        }
    }
    assert_eq!((positive, negative), (8, 12));
}

#[test]
fn wide_primary_declarations_keep_exact_carriers_no_inferred_defaults_and_existing_row_ceiling() {
    for family in [Family::Int64, Family::Currency] {
        let columns = columns(family);
        assert_eq!(columns[0].primary_key_name(), Some("PrimaryId"));
        assert_eq!(columns[0].required(), None);
        assert_eq!(columns[0].default_literal(), None);
        assert!(matches!(
            columns[0].with_default_literal(&Value::Int32(7)),
            Err(CreateDatabaseError::Carrier),
        ));
        let wrong = match family {
            Family::Int64 => Family::Currency.value(1),
            Family::Currency => Family::Int64.value(1),
        };
        for key in [wrong, Value::Int32(1)] {
            let row = [key, Value::Int32(10)];
            assert!(matches!(
                create_ace16_database("Items", &columns, &[&row], &mut budget(), &|| false),
                Err(CreateDatabaseError::Carrier),
            ));
        }
        let too_many = (1..=65)
            .map(|key| vec![family.value(key), Value::Int32(10)])
            .collect::<Vec<_>>();
        let rows = too_many.iter().map(Vec::as_slice).collect::<Vec<_>>();
        assert!(matches!(
            create_ace16_database("Items", &columns, &rows, &mut budget(), &|| false),
            Err(CreateDatabaseError::RowLimit),
        ));
        let two = [columns[0], columns[1].with_primary_key("Second").unwrap()];
        assert!(matches!(
            create_ace16_database("Items", &two, &[], &mut budget(), &|| false),
            Err(CreateDatabaseError::Declaration),
        ));
    }
}

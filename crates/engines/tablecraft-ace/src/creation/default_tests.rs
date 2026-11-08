//! Matching creators consume independent source bytes after source qualification.
use super::{
    CreateColumn, CreateDatabaseError, CreateTable, create_ace16_database,
    create_ace16_database_tables,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_schema::ScalarType;
use tablecraft_testkit::fixtures::access::canonical_creation_defaults as sources;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

#[test]
fn explicit_default_request_retains_null_presence_and_exact_int32_without_coercion() {
    let absent = CreateColumn::new("Score", ScalarType::Int32);
    assert!(absent.default_literal().is_none());
    for literal in [
        Value::Null,
        Value::Int32(i32::MIN),
        Value::Int32(0),
        Value::Int32(i32::MAX),
    ] {
        let column = absent.with_default_literal(&literal).unwrap();
        assert!(std::ptr::eq(column.default_literal().unwrap(), &literal));
        assert_eq!(column.required(), None);
        assert_eq!(column.allow_zero_length(), None);
    }
    for wrong in [
        Value::Boolean(false),
        Value::Int16(7),
        Value::Int64(7),
        Value::Text("7".into()),
    ] {
        assert!(matches!(
            absent.with_default_literal(&wrong),
            Err(CreateDatabaseError::Carrier)
        ));
    }
    let null = Value::Null;
    for scalar in [ScalarType::Text, ScalarType::Binary] {
        assert!(matches!(
            CreateColumn::new("Other", scalar).with_default_literal(&null),
            Err(CreateDatabaseError::Declaration)
        ));
    }
    let text = CreateColumn::short_text("Other", 8)
        .unwrap()
        .with_default_literal(&null)
        .unwrap();
    assert!(std::ptr::eq(text.default_literal().unwrap(), &null));
    assert!(matches!(
        CreateColumn::new("Flag", ScalarType::Boolean).with_default_literal(&null),
        Err(CreateDatabaseError::Carrier)
    ));
}

#[test]
fn both_creators_match_all_six_complete_default_sources_before_any_default_execution() {
    let mut outputs = 0;
    for case in sources::CASES {
        let source = sources::source(case);
        let literal = source.omitted_score.map_or(Value::Null, Value::Int32);
        let mut score = CreateColumn::new("Score", ScalarType::Int32);
        if source.expression.is_some() {
            score = score.with_default_literal(&literal).unwrap();
        }
        if let Some(required) = source.required {
            score = score.with_required(required);
        }
        let columns = [CreateColumn::new("Id", ScalarType::Int32), score];
        let rows = source.rows.iter().map(Vec::as_slice).collect::<Vec<_>>();
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
    assert_eq!(outputs, 12);
}

#[path = "default_tests/resources.rs"]
mod resources;

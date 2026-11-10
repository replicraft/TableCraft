use crate::{
    CreateColumn, CreateDatabaseError, FirstTableDefinition, FirstTableError, NativeImage,
};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;
use tablecraft_testkit::fixtures::access::canonical_wide_table_creation as raw;

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 1_000_000, 100_000_000)
}

fn capture(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(7301),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn declarations<'a>(names: &'a [String], zero: &'a Value) -> Vec<CreateColumn<'a>> {
    names
        .iter()
        .enumerate()
        .map(|(position, name)| {
            let column = CreateColumn::new(name, ScalarType::Int32);
            if position == 0 {
                column
                    .with_primary_key("PrimaryKey")
                    .unwrap()
                    .with_incrementing_autonumber()
                    .unwrap()
            } else {
                column
                    .with_required(false)
                    .with_default_literal(zero)
                    .unwrap()
            }
        })
        .collect()
}

#[test]
fn native_creation_matches_independent_255_column_definition_and_external_properties() {
    let source = raw::source();
    let names = (0..255).map(raw::field_name).collect::<Vec<_>>();
    let zero = Value::Int32(0);
    let columns = declarations(&names, &zero);
    assert!(matches!(
        FirstTableDefinition::capture("Table1", &columns, &mut budget(), &|| false),
        Err(CreateDatabaseError::ColumnLimit)
    ));
    let definition =
        FirstTableDefinition::capture_native("Table1", &columns, &mut budget(), &|| false).unwrap();
    assert!(definition.is_native());
    let plan = capture(&source.before)
        .plan_first_table_native(definition, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(plan.after_bytes().len(), source.after.len());
    assert_eq!(
        plan.after_bytes()
            .iter()
            .zip(&source.after)
            .position(|(left, right)| left != right),
        None
    );
    assert_eq!(plan.before_bytes(), source.before);
}

#[test]
fn original_planner_rejects_native_request_even_when_its_column_count_fits_original() {
    let source = raw::source();
    let columns = [CreateColumn::new("ID", ScalarType::Int32)
        .with_primary_key("PrimaryKey")
        .unwrap()
        .with_incrementing_autonumber()
        .unwrap()];
    let definition =
        FirstTableDefinition::capture_native("Table1", &columns, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        capture(&source.before).plan_first_table(definition, &mut budget(), &|| false),
        Err(FirstTableError::Create(CreateDatabaseError::Declaration))
    ));
}

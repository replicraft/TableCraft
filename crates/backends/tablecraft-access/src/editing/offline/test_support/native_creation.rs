//! Actual native declarations shared by first-table and append session tests.
use tablecraft_ace::{CreateColumn, FirstTableDefinition};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_schema::ScalarType;
use tablecraft_testkit::fixtures::access::canonical_wide_table_creation as raw;

pub(in crate::editing::offline) fn wide_definition(
    table: &str,
    budget: &mut ResourceBudget,
) -> FirstTableDefinition {
    let names: Vec<_> = (0..255).map(raw::field_name).collect();
    let zero = Value::Int32(0);
    let mut columns = vec![
        CreateColumn::new(&names[0], ScalarType::Int32)
            .with_primary_key("PrimaryKey")
            .unwrap()
            .with_incrementing_autonumber()
            .unwrap(),
    ];
    columns.extend(names[1..].iter().map(|name| {
        CreateColumn::new(name, ScalarType::Int32)
            .with_required(false)
            .with_default_literal(&zero)
            .unwrap()
    }));
    FirstTableDefinition::capture_native(table, &columns, budget, &|| false).unwrap()
}

use super::prepare;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_query::{
    ExecutionOptions, prepare as bind,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::ParseLimits;

#[test]
fn ordinary_text_dependencies_are_deduplicated_and_concatenation_executes() {
    let columns = vec![
        Column::new("FirstName".into(), ScalarType::Text, true).unwrap(),
        Column::new("LastName".into(), ScalarType::Text, true).unwrap(),
    ];
    let (query, dependencies) = prepare(
        "Contacts",
        "[FirstName] & ' ' & [LastName] & [FIRSTNAME]",
        &columns,
        ParseLimits::default(),
        &mut ResourceBudget::new(1024 * 1024, 100, 10000),
        &|| false,
    )
    .unwrap();
    assert_eq!(dependencies, ["FirstName", "LastName"]);
    let source = MemorySource::new(vec![
        MemoryTable::new(
            "Contacts".into(),
            columns,
            vec![vec![
                Value::Text("Ada".into()),
                Value::Text("Lovelace".into()),
            ]],
        )
        .unwrap(),
    ])
    .unwrap();
    let result = bind(&query, &source)
        .unwrap()
        .execute(&source, &[], ExecutionOptions::default())
        .unwrap();
    assert_eq!(result.rows(), [vec![Value::Text("Ada LovelaceAda".into())]]);
}

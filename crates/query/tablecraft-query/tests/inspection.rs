use tablecraft_core::values::Value;
use tablecraft_expressions::{EvalError, functions::conversions::text::NumberFormat};
use tablecraft_query::{
    ExecutionOptions, QueryError, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::{ParseLimits, parse_query};
fn source() -> MemorySource {
    MemorySource::new(vec![
        MemoryTable::new(
            "Items".into(),
            vec![
                Column::new("Unused".into(), ScalarType::Int32, false).unwrap(),
                Column::new("Id".into(), ScalarType::Int32, false).unwrap(),
                Column::new("Label".into(), ScalarType::Text, true).unwrap(),
            ],
            vec![
                vec![Value::Int32(99), Value::Int32(1), Value::Text("53".into())],
                vec![
                    Value::Int32(99),
                    Value::Int32(2),
                    Value::Text("45 Help".into()),
                ],
                vec![Value::Int32(99), Value::Int32(3), Value::Null],
            ],
        )
        .unwrap(),
    ])
    .unwrap()
}
fn run(sql: &str, source: &MemorySource) -> Result<Vec<Vec<Value>>, QueryError> {
    let query = parse_query(sql, ParseLimits::default()).unwrap();
    Ok(prepare(query.query(), source)?
        .execute(
            source,
            &[],
            ExecutionOptions {
                number_format: Some(NumberFormat::Invariant),
                ..Default::default()
            },
        )?
        .rows()
        .to_vec())
}
#[test]
fn inspections_preserve_pruned_fields_group_keys_aggregates_and_correlations() {
    let source = source();
    assert_eq!(
        run(
            "SELECT Id FROM Items WHERE IsNumeric(Label) ORDER BY Id",
            &source
        )
        .unwrap(),
        [vec![Value::Int32(1)]]
    );
    let grouped = run(
        "SELECT IsNumeric(Label),Count(*) FROM Items GROUP BY IsNumeric(Label)",
        &source,
    )
    .unwrap();
    assert!(grouped.contains(&vec![Value::Boolean(true), Value::Int64(1)]));
    assert!(grouped.contains(&vec![Value::Boolean(false), Value::Int64(2)]));
    assert_eq!(
        run("SELECT IsNumeric(Max(Id)) FROM Items", &source).unwrap(),
        [vec![Value::Boolean(true)]]
    );
    assert_eq!(
        run(
            "SELECT Id FROM Items AS A WHERE (SELECT IsNumeric(A.Label)) ORDER BY Id",
            &source
        )
        .unwrap(),
        [vec![Value::Int32(1)]]
    );
    assert_eq!(
        run("SELECT IsNumeric(Label) FROM Items GROUP BY Id", &source).err(),
        Some(QueryError::Grouping)
    );
    assert_eq!(
        run(
            "SELECT Count(*) FROM Items WHERE NOT IsNumeric(Label)",
            &source
        )
        .unwrap(),
        [vec![Value::Int64(2)]]
    );
}
#[test]
fn inspection_parameters_use_each_execution_policy_and_keep_eager_errors() {
    let source = source();
    let query = parse_query(
        "PARAMETERS P TEXT; SELECT IsNumeric(P)",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    for (format, expected) in [
        (NumberFormat::EnUs, true),
        (NumberFormat::DeDe, false),
        (NumberFormat::Invariant, false),
    ] {
        assert_eq!(
            plan.execute(
                &source,
                &[("P", Value::Text("$1".into()))],
                ExecutionOptions {
                    number_format: Some(format),
                    ..Default::default()
                }
            )
            .unwrap()
            .rows(),
            [vec![Value::Boolean(expected)]]
        );
    }
    assert_eq!(
        plan.execute(
            &source,
            &[("P", Value::Text("$1".into()))],
            ExecutionOptions::default()
        )
        .err(),
        Some(QueryError::Evaluation(EvalError::NumberFormatRequired))
    );
    for sql in [
        "SELECT IsNumeric(1/0)",
        "SELECT IsNumeric(*)",
        "SELECT IsNumeric()",
        "SELECT IsNumeric('1',1/0)",
        "SELECT VBA.IsNumeric('1')",
        "SELECT Id FROM Items WHERE IsNumeric(Label) AND CLng(Label)>0",
    ] {
        assert!(run(sql, &source).is_err(), "{sql}");
    }
}

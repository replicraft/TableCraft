use tablecraft_core::values::Value;
use tablecraft_query::{
    ExecutionOptions, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::{ParseLimits, parse_query};
fn source() -> MemorySource {
    MemorySource::new(vec![
        MemoryTable::new(
            "Items".into(),
            vec![Column::new("N".into(), ScalarType::Int32, true).unwrap()],
            vec![
                vec![Value::Int32(459)],
                vec![Value::Int32(459)],
                vec![Value::Null],
            ],
        )
        .unwrap(),
    ])
    .unwrap()
}
#[test]
fn numeric_text_calls_preserve_fields_parameters_grouping_and_aggregates() {
    let source = source();
    let query = parse_query(
        "SELECT N,Hex(N),Oct(N),Str(N) FROM Items GROUP BY N",
        ParseLimits::default(),
    )
    .unwrap();
    let rows = prepare(query.query(), &source)
        .unwrap()
        .execute(&source, &[], ExecutionOptions::default())
        .unwrap();
    assert!(rows.rows().contains(&vec![
        Value::Int32(459),
        Value::Text("1CB".into()),
        Value::Text("713".into()),
        Value::Text(" 459".into())
    ]));
    assert!(
        rows.rows()
            .contains(&vec![Value::Null, Value::Null, Value::Null, Value::Null])
    );
    let query = parse_query(
        "PARAMETERS P LONG; SELECT Hex(P),Oct(P),Str(P),(SELECT Str(Sum(N)) FROM Items)",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    for (value, hex, oct, decimal) in [(459, "1CB", "713", " 459"), (-1, "FFFF", "177777", "-1")] {
        assert_eq!(
            plan.execute(
                &source,
                &[("P", Value::Int32(value))],
                ExecutionOptions::default()
            )
            .unwrap()
            .rows(),
            [vec![
                Value::Text(hex.into()),
                Value::Text(oct.into()),
                Value::Text(decimal.into()),
                Value::Text(" 918".into())
            ]]
        );
    }
}
#[test]
fn numeric_text_calls_keep_eager_errors_and_validated_shapes() {
    let source = source();
    for sql in [
        "SELECT IIf(False,Hex('bad'),0)",
        "SELECT Hex()",
        "SELECT Oct(1,1/0)",
        "SELECT VBA.Str(1)",
        "SELECT Hex(2147483648.0)",
    ] {
        let query = parse_query(sql, ParseLimits::default()).unwrap();
        assert!(
            prepare(query.query(), &source)
                .and_then(|plan| plan.execute(&source, &[], ExecutionOptions::default()))
                .is_err(),
            "{sql}"
        );
    }
}

#[test]
fn numeric_text_expansion_uses_the_host_query_work_limit() {
    let source = MemorySource::default();
    let query = parse_query("SELECT Str(CDbl(1e-299))", ParseLimits::default()).unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    let options = ExecutionOptions {
        limits: tablecraft_query::QueryLimits {
            steps: 100,
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(matches!(plan.execute(&source, &[], options),
        Err(tablecraft_query::QueryError::Evaluation(tablecraft_expressions::EvalError::Limit(error)))
            if error.resource == tablecraft_core::limits::Resource::Steps));
    assert!(
        plan.execute(&source, &[], ExecutionOptions::default())
            .is_ok()
    );
}

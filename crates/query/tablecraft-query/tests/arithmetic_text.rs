use tablecraft_core::values::{Decimal, Double, Value};
use tablecraft_expressions::{EvalError, functions::conversions::text::NumberFormat};
use tablecraft_query::{
    ExecutionOptions, QueryError, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::parse_query;

fn run(sql: &str, format: Option<NumberFormat>) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = MemorySource::new(Vec::new()).unwrap();
    let syntax = parse_query(sql, Default::default()).unwrap();
    Ok(prepare(syntax.query(), &source)?
        .execute(
            &source,
            &[],
            ExecutionOptions {
                number_format: format,
                ..Default::default()
            },
        )?
        .rows()
        .to_vec())
}
fn double(value: f64) -> Value {
    Value::Double(Double::from_float(value))
}

#[test]
fn arithmetic_text_keeps_numeric_routes_and_string_concatenation_separate() {
    assert_eq!(run("SELECT '2,5'+3,3-'2,5','2,5'*'4','2,5'/2,'9,5' Mod '2,5','9007199254740993' \\ 1,-'2,5',+'2,5','2'+'3','bad'+NULL,CDec(1)+'9007199254740993,125'", Some(NumberFormat::DeDe)).unwrap(), [vec![
        double(5.5), double(0.5), double(10.0), double(1.25), Value::Int32(0), Value::Int64(9007199254740993), double(-2.5), double(2.5), Value::Text("23".into()), Value::Null, Value::Decimal(Decimal::new(9007199254740994125, 3).unwrap())
    ]]);
    assert_eq!(
        run("SELECT '2'+1", None).err(),
        Some(QueryError::Evaluation(EvalError::NumberFormatRequired))
    );
    assert_eq!(
        run("SELECT 'bad'*2", Some(NumberFormat::EnUs)).err(),
        Some(QueryError::Evaluation(EvalError::TypeMismatch))
    );
    assert_eq!(
        run("SELECT 'bad'+(1/0)", Some(NumberFormat::EnUs)).err(),
        Some(QueryError::Evaluation(EvalError::DivisionByZero))
    );
    assert_eq!(
        run("SELECT Null+('2'/0)", Some(NumberFormat::EnUs)).err(),
        Some(QueryError::Evaluation(EvalError::DivisionByZero))
    );
}

#[test]
fn numeric_text_flows_through_derived_rows_aggregates_domains_and_dynamic_eval() {
    let result = run(
        "SELECT Sum(N*'2'),DSum('CDbl(\"2.5\")+1','Local'),Eval(\"'2.5'+1\") FROM (SELECT '2.5' AS N UNION ALL SELECT '1.25') AS Q",
        Some(NumberFormat::EnUs),
    );
    // An unknown domain remains an error; arithmetic never supplies ambient data.
    assert_eq!(result.err(), Some(QueryError::UnknownName));
    let source = MemorySource::new(vec![
        MemoryTable::new(
            "T".into(),
            vec![Column::new("N".into(), ScalarType::Text, false).unwrap()],
            vec![
                vec![Value::Text("2.5".into())],
                vec![Value::Text("1.25".into())],
            ],
        )
        .unwrap(),
    ])
    .unwrap();
    let syntax = parse_query(
        r#"SELECT DSum("N*'2'","T"),DLookup("N*'2'","T","N*'2'>4")"#,
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    assert_eq!(
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                number_format: Some(NumberFormat::EnUs),
                ..Default::default()
            }
        )
        .unwrap()
        .rows(),
        [vec![double(7.5), double(5.0)]]
    );

    assert_eq!(run("SELECT Sum(N*'2'),Eval(\"'2.5'+1\") FROM (SELECT '2.5' AS N UNION ALL SELECT '1.25') AS Q HAVING Sum(N*'2')>7", Some(NumberFormat::EnUs)).unwrap(), [vec![double(7.5), double(3.5)]]);
    assert_eq!(
        run(
            "SELECT N FROM (SELECT '2.5' AS N UNION ALL SELECT '1.25') AS Q WHERE N*'2'>4",
            Some(NumberFormat::EnUs)
        )
        .unwrap(),
        [vec![Value::Text("2.5".into())]]
    );
    assert_eq!(run("TRANSFORM Sum(N*'2') SELECT G FROM (SELECT 1 AS G,1 AS P,'2.5' AS N UNION ALL SELECT 1,1,'1.25') AS Q GROUP BY G PIVOT P IN (1)", Some(NumberFormat::EnUs)).unwrap(), [vec![Value::Int64(1), double(7.5)]]);
}

#[test]
fn prepared_numeric_text_operators_use_each_executions_number_policy() {
    let source = MemorySource::new(Vec::new()).unwrap();
    let syntax = parse_query("SELECT '1,234'+1", Default::default()).unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    for (format, expected) in [(NumberFormat::EnUs, 1235.0), (NumberFormat::DeDe, 2.234)] {
        assert_eq!(
            plan.execute(
                &source,
                &[],
                ExecutionOptions {
                    number_format: Some(format),
                    ..Default::default()
                }
            )
            .unwrap()
            .rows(),
            [vec![double(expected)]]
        );
    }
}

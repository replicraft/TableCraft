use tablecraft_core::values::{Decimal, Value};
use tablecraft_expressions::EvalError;
use tablecraft_query::{
    ExecutionOptions, QueryError, Source, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::{ParseLimits, parse_query};

fn source() -> MemorySource {
    MemorySource::new(vec![
        MemoryTable::new(
            "T".into(),
            ["Unused", "N"]
                .map(|name| Column::new(name.into(), ScalarType::Decimal, true).unwrap())
                .to_vec(),
            [-84, -25, 25, 84]
                .into_iter()
                .map(|n| vec![Value::Null, Value::Decimal(Decimal::new(n, 1).unwrap())])
                .collect(),
        )
        .unwrap(),
    ])
    .unwrap()
}
fn run(sql: &str, source: &dyn Source) -> Result<Vec<Vec<Value>>, QueryError> {
    let parsed = parse_query(sql, ParseLimits::default()).unwrap();
    Ok(prepare(parsed.query(), source)?
        .execute(source, &[], ExecutionOptions::default())?
        .rows()
        .to_vec())
}
fn decimal(n: i128) -> Value {
    Value::Decimal(Decimal::new(n, 0).unwrap())
}
#[test]
fn math_functions_bind_prune_filter_and_group_by_function_identity() {
    let source = source();
    assert_eq!(
        run(
            "SELECT Int(N),Fix(N),Sgn(N),Abs(N) FROM T WHERE Sgn(N)<0 ORDER BY 1",
            &source
        )
        .unwrap(),
        [
            vec![
                decimal(-9),
                decimal(-8),
                Value::Int16(-1),
                Value::Decimal(Decimal::new(84, 1).unwrap())
            ],
            vec![
                decimal(-3),
                decimal(-2),
                Value::Int16(-1),
                Value::Decimal(Decimal::new(25, 1).unwrap())
            ]
        ]
    );
    assert_eq!(
        run(
            "SELECT Abs(N),Count(*) FROM T GROUP BY Abs(N) ORDER BY 1",
            &source
        )
        .unwrap(),
        [
            vec![
                Value::Decimal(Decimal::new(25, 1).unwrap()),
                Value::Int64(2)
            ],
            vec![
                Value::Decimal(Decimal::new(84, 1).unwrap()),
                Value::Int64(2)
            ]
        ]
    );
    assert_eq!(
        run("SELECT Fix(N) FROM T GROUP BY Int(N)", &source).err(),
        Some(QueryError::Grouping)
    );
    assert_eq!(
        run("SELECT Round(Sum(N)) FROM T", &source).unwrap(),
        [vec![decimal(0)]]
    );
}
#[test]
fn round_is_rebound_in_parameters_correlated_sources_and_crosstab_cells() {
    let source = source();
    let parsed = parse_query(
        "PARAMETERS Places LONG; SELECT Round(N,Places) AS R FROM T ORDER BY R",
        ParseLimits::default(),
    )
    .unwrap();
    let prepared = prepare(parsed.query(), &source).unwrap();
    assert_eq!(
        prepared
            .execute(
                &source,
                &[("Places", Value::Int32(0))],
                ExecutionOptions::default()
            )
            .unwrap()
            .rows(),
        [
            vec![decimal(-8)],
            vec![decimal(-2)],
            vec![decimal(2)],
            vec![decimal(8)]
        ]
    );
    assert_eq!(
        prepared
            .execute(
                &source,
                &[("Places", Value::Int32(1))],
                ExecutionOptions::default()
            )
            .unwrap()
            .rows()
            .len(),
        4
    );
    assert_eq!(
        run("SELECT (SELECT Abs(A.N)) FROM T AS A ORDER BY 1", &source)
            .unwrap()
            .len(),
        4
    );
    assert_eq!(run("TRANSFORM Round(Sum(N)) SELECT Sgn(N) AS Sign FROM T GROUP BY Sgn(N) ORDER BY Sign PIVOT Abs(N) IN (CDec(2.5),CDec(8.4))",&source).err(),Some(QueryError::Unsupported));
    let result=run("TRANSFORM Round(Sum(N)) SELECT Sgn(N) AS Sign FROM T GROUP BY Sgn(N) ORDER BY Sign PIVOT CInt(Abs(N)) IN (2,8)",&source).unwrap();
    assert_eq!(
        result,
        [
            vec![Value::Int16(-1), decimal(-2), decimal(-8)],
            vec![Value::Int16(1), decimal(2), decimal(8)]
        ]
    );
}
#[test]
fn invalid_arities_qualifiers_and_eager_errors_are_explicit() {
    let source = source();
    for sql in [
        "SELECT Abs() FROM T",
        "SELECT Abs(N,1) FROM T",
        "SELECT Round(N,,) FROM T",
        "SELECT VBA.Abs(N) FROM T",
    ] {
        let parsed = parse_query(sql, ParseLimits::default()).unwrap();
        assert_eq!(
            prepare(parsed.query(), &source).err(),
            Some(QueryError::Unsupported),
            "{sql}"
        );
    }
    assert_eq!(
        run("SELECT Round(NULL,1/0) FROM T", &source).err(),
        Some(QueryError::Evaluation(EvalError::DivisionByZero))
    );
    assert_eq!(
        run("SELECT IIf(True,1,Abs(CInt(-32768))) FROM T", &source).err(),
        Some(QueryError::Evaluation(EvalError::Overflow))
    );
    let parsed = parse_query("SELECT Round(N) FROM T", ParseLimits::default()).unwrap();
    assert_eq!(
        prepare(parsed.query(), &source)
            .unwrap()
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    cancelled: Some(&|| true),
                    ..Default::default()
                }
            )
            .err(),
        Some(QueryError::Cancelled)
    );
}

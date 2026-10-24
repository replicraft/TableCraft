#[path = "statistics/admission.rs"]
mod admission;
#[path = "statistics/bounds.rs"]
mod bounds;
#[path = "statistics/fixed.rs"]
mod fixed;
#[path = "statistics/numerical.rs"]
mod numerical;
use proptest::prelude::*;
use tablecraft_core::values::{Double, Value};
use tablecraft_expressions::EvalError;
use tablecraft_query::{
    ExecutionOptions, QueryError, QueryResult, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::{ParseLimits, parse_query};

fn source(kind: ScalarType, values: Vec<Value>) -> MemorySource {
    MemorySource::new(vec![
        MemoryTable::new(
            "T".into(),
            vec![
                Column::new("X".into(), kind, true).unwrap(),
                Column::new("G".into(), ScalarType::Int32, false).unwrap(),
            ],
            values
                .into_iter()
                .enumerate()
                .map(|(i, v)| vec![v, Value::Int32((i % 2) as i32)])
                .collect(),
        )
        .unwrap(),
    ])
    .unwrap()
}
fn ints(values: &[Option<i32>]) -> MemorySource {
    source(
        ScalarType::Int32,
        values
            .iter()
            .map(|v| v.map_or(Value::Null, Value::Int32))
            .collect(),
    )
}
fn run(sql: &str, source: &MemorySource) -> Result<QueryResult, QueryError> {
    let parsed = parse_query(sql, ParseLimits::default()).unwrap();
    prepare(parsed.query(), source)?.execute(source, &[], ExecutionOptions::default())
}
fn float(value: &Value) -> f64 {
    let Value::Double(value) = value else {
        panic!("expected Double")
    };
    value.to_float()
}
fn near(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= expected.abs().max(1e-300) * 2e-13,
        "{actual:?} != {expected:?}"
    );
}
#[test]
fn global_sample_population_null_and_singleton_rules_follow_each_function() {
    let data = ints(&[
        Some(2),
        Some(4),
        Some(4),
        Some(4),
        Some(5),
        Some(5),
        Some(7),
        Some(9),
        None,
    ]);
    let result = run("SELECT Var(X),VarP(X),StDev(X),StDevP(X) FROM T", &data).unwrap();
    for (actual, expected) in
        result.rows()[0]
            .iter()
            .zip([32.0_f64 / 7.0, 4.0, (32.0_f64 / 7.0).sqrt(), 2.0])
    {
        near(float(actual), expected);
    }
    for values in [vec![], vec![None], vec![Some(7)], vec![Some(7), None]] {
        let result = run(
            "SELECT Var(X),VarP(X),StDev(X),StDevP(X) FROM T",
            &ints(&values),
        )
        .unwrap();
        let mut expected = vec![Value::Null; 4];
        if values.iter().any(Option::is_some) {
            expected[3] = Value::Double(Double::from_float(0.0));
        }
        assert_eq!(result.rows(), [expected]);
    }
    assert_eq!(
        run("SELECT StDevP(4)", &ints(&[])).unwrap().rows(),
        [vec![Value::Double(Double::from_float(0.0))]]
    );
}
#[test]
fn grouping_having_ordering_nested_reads_unions_parameters_and_crosstabs_use_existing_plans() {
    let data = ints(&[Some(1), Some(2), Some(3), Some(4), Some(5), Some(6)]);
    let result = run(
        "SELECT G,VarP(X) AS V FROM T GROUP BY G HAVING StDevP(X)>1 ORDER BY V DESC,G",
        &data,
    )
    .unwrap();
    assert_eq!(result.rows().len(), 2);
    for row in result.rows() {
        near(float(&row[1]), 8.0 / 3.0);
    }
    assert_eq!(
        run("SELECT D.V FROM (SELECT Var(X) AS V FROM T) AS D", &data)
            .unwrap()
            .rows(),
        run("SELECT Var(X) FROM T", &data).unwrap().rows()
    );
    let result = run(
        "SELECT (SELECT VarP(B.X) FROM T AS B WHERE B.G=A.G) FROM T AS A",
        &data,
    )
    .unwrap();
    for row in result.rows() {
        near(float(&row[0]), 8.0 / 3.0);
    }
    let parsed = parse_query(
        "PARAMETERS P LONG; SELECT Var(X+P),StDevP(X+P) FROM T",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &data).unwrap();
    for value in [1, 2000] {
        let result = plan
            .execute(&data, &[("P", Value::Int32(value))], Default::default())
            .unwrap();
        near(float(&result.rows()[0][0]), 3.5);
        near(float(&result.rows()[0][1]), (35.0_f64 / 12.0).sqrt());
    }
    let result = run(
        "SELECT VarP(X) FROM T UNION ALL SELECT StDevP(X) FROM T",
        &data,
    )
    .unwrap();
    assert_eq!(result.rows().len(), 2);
    let result = run(
        "TRANSFORM VarP(X) SELECT 1 AS R FROM T GROUP BY 1 PIVOT G IN (0,1)",
        &data,
    )
    .unwrap();
    near(float(&result.rows()[0][1]), 8.0 / 3.0);
    near(float(&result.rows()[0][2]), 8.0 / 3.0);
}
#[test]
fn invalid_shapes_and_late_argument_errors_fail_before_any_partial_result() {
    let data = ints(&[Some(1), Some(0)]);
    for sql in [
        "SELECT Var(*) FROM T",
        "SELECT StDev() FROM T",
        "SELECT VarP(X,X) FROM T",
        "SELECT Var(Avg(X)) FROM T",
        "SELECT Foo.Var(X) FROM T",
        "SELECT X FROM T WHERE StDevP(X)>0",
    ] {
        let parsed = parse_query(sql, ParseLimits::default()).unwrap();
        assert!(prepare(parsed.query(), &data).is_err(), "{sql}");
    }
    for sql in [
        "SELECT TOP 1 Var(1 \\ X) FROM T",
        "SELECT IIf(False,StDevP(1 \\ X),0) FROM T",
    ] {
        assert_eq!(
            run(sql, &data).err(),
            Some(EvalError::DivisionByZero.into()),
            "{sql}"
        );
    }
    assert_eq!(
        run("SELECT VarP('text') FROM T", &data).err(),
        Some(EvalError::TypeMismatch.into())
    );
}
proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]
    #[test]
    fn finite_integer_groups_match_an_independent_exact_rational_model(values in prop::collection::vec(-10000_i32..10000,2..35)) {
        let n=values.len() as i128;let sum=values.iter().map(|v|i128::from(*v)).sum::<i128>();let squares=values.iter().map(|v|i128::from(*v).pow(2)).sum::<i128>();let numerator=n*squares-sum*sum;
        let data=ints(&values.iter().copied().map(Some).collect::<Vec<_>>());let result=run("SELECT Var(X),VarP(X),StDev(X),StDevP(X) FROM T",&data).unwrap();
        let sample=numerator as f64/(n*(n-1)) as f64;let population=numerator as f64/(n*n) as f64;
        for (actual,expected) in result.rows()[0].iter().zip([sample,population,sample.sqrt(),population.sqrt()]) {near(float(actual),expected);}
    }
}

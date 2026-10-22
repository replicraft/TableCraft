use tablecraft_core::values::{Currency, Decimal, Double, Value};
use tablecraft_query::{
    ExecutionOptions, OutputType, QueryError, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType, TypeDescriptor};
use tablecraft_sql::{ParseLimits, parse_query};

fn source(values: &[Option<i64>]) -> MemorySource {
    MemorySource::new(vec![
        MemoryTable::new(
            "T".into(),
            vec![Column::new("Money".into(), ScalarType::Currency, true).unwrap()],
            values
                .iter()
                .map(|value| {
                    vec![value.map_or(Value::Null, |n| Value::Currency(Currency::from_scaled(n)))]
                })
                .collect(),
        )
        .unwrap(),
    ])
    .unwrap()
}
fn run(values: &[Option<i64>], sql: &str) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = source(values);
    let parsed = parse_query(sql, ParseLimits::default()).unwrap();
    let plan = prepare(parsed.query(), &source)?;
    let result = plan.execute(&source, &[], ExecutionOptions::default())?;
    let expected_type = if sql.contains("DAvg") {
        OutputType::Dynamic
    } else {
        OutputType::Scalar {
            data_type: TypeDescriptor::new(ScalarType::Double),
            nullable: true,
        }
    };
    assert_eq!(result.output_types(), [expected_type]);
    Ok(result.rows().to_vec())
}

#[test]
fn currency_means_ignore_nulls_and_support_domain_calls() {
    for sql in ["SELECT Avg([Money]) FROM T", "SELECT DAvg('[Money]','T')"] {
        assert_eq!(
            run(&[Some(12500), None, Some(25000)], sql).unwrap(),
            [vec![Value::Double(Double::from_float(1.875))]]
        );
        assert_eq!(run(&[], sql).unwrap(), [vec![Value::Null]]);
        assert_eq!(run(&[None], sql).unwrap(), [vec![Value::Null]]);
    }
}

#[test]
fn currency_means_do_not_overflow_an_intermediate_currency_sum() {
    for (values, expected) in [
        ([Some(i64::MAX), Some(i64::MAX)], 922337203685477.6),
        ([Some(i64::MIN), Some(i64::MIN)], -922337203685477.6),
        ([Some(i64::MAX), Some(i64::MIN)], -0.00005),
    ] {
        assert_eq!(
            run(&values, "SELECT Avg([Money]) FROM T").unwrap(),
            [vec![Value::Double(Double::from_float(expected))]]
        );
    }
}

#[test]
fn integral_means_widen_totals_before_division() {
    let source = MemorySource::new(vec![
        MemoryTable::new(
            "T".into(),
            vec![Column::new("Amount".into(), ScalarType::Int64, false).unwrap()],
            vec![vec![Value::Int64(i64::MAX)], vec![Value::Int64(i64::MAX)]],
        )
        .unwrap(),
    ])
    .unwrap();
    let parsed = parse_query("SELECT Avg(Amount) FROM T", ParseLimits::default()).unwrap();
    let result = prepare(parsed.query(), &source)
        .unwrap()
        .execute(&source, &[], ExecutionOptions::default())
        .unwrap();
    assert_eq!(
        result.rows(),
        [vec![Value::Double(Double::from_float(
            9223372036854775808.0
        ))]]
    );
}

#[test]
fn mixed_decimal_currency_and_floating_means_keep_explicit_decimal_precedence() {
    let source = MemorySource::new(vec![]).unwrap();
    for (input, expected) in [
        (
            "SELECT CCur(1.25) AS Amount UNION ALL SELECT CDbl(2.5)",
            Value::Double(Double::from_float(1.875)),
        ),
        (
            "SELECT CDbl(2.5) AS Amount UNION ALL SELECT CCur(1.25)",
            Value::Double(Double::from_float(1.875)),
        ),
        (
            "SELECT CCur(1.25) AS Amount UNION ALL SELECT CDec(2.5)",
            Value::Decimal(Decimal::new(18750, 4).unwrap()),
        ),
        (
            "SELECT CDbl(2.5) AS Amount UNION ALL SELECT CDec(1.25)",
            Value::Decimal(Decimal::new(18750, 4).unwrap()),
        ),
    ] {
        let sql = format!("SELECT Avg(Amount) FROM ({input}) AS Inputs");
        let parsed = parse_query(&sql, ParseLimits::default()).unwrap();
        let result = prepare(parsed.query(), &source)
            .unwrap()
            .execute(&source, &[], ExecutionOptions::default())
            .unwrap();
        assert_eq!(result.rows(), [vec![expected]]);
    }
}

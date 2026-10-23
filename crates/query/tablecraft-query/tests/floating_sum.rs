use std::cell::Cell;
use tablecraft_core::{
    limits::Resource,
    values::{Double, Value},
};
use tablecraft_expressions::{EvalError, functions::math::random::RndState};
use tablecraft_query::{
    ExecutionOptions, QueryError, QueryLimits, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::parse_query;

fn source(values: &[f64]) -> MemorySource {
    MemorySource::new(vec![
        MemoryTable::new(
            "T".into(),
            vec![Column::new("Amount".into(), ScalarType::Double, true).unwrap()],
            values
                .iter()
                .map(|value| vec![Value::Double(Double::from_float(*value))])
                .collect(),
        )
        .unwrap(),
    ])
    .unwrap()
}

fn total(values: &[f64], sql: &str) -> Result<f64, QueryError> {
    let source = source(values);
    let parsed = parse_query(sql, Default::default()).unwrap();
    let result = prepare(parsed.query(), &source)?.execute(&source, &[], Default::default())?;
    let Value::Double(value) = result.rows()[0][0] else {
        panic!("sum lost its Double carrier");
    };
    Ok(value.to_float())
}

#[test]
fn exact_sums_keep_residuals_and_only_final_floating_overflow_fails() {
    for (values, expected) in [
        (vec![f64::MAX, f64::MAX, -f64::MAX], f64::MAX),
        (vec![-f64::MAX, -f64::MAX, f64::MAX], -f64::MAX),
        (vec![f64::MAX, 1.0, -f64::MAX], 1.0),
        (vec![1e308, 1e-300, -1e308], 1e-300),
        (
            vec![f64::from_bits(1), f64::from_bits(1)],
            f64::from_bits(2),
        ),
        (vec![1.0, f64::EPSILON / 2.0], 1.0),
        (
            vec![1.0, f64::EPSILON, f64::EPSILON / 2.0],
            1.0 + 2.0 * f64::EPSILON,
        ),
        (vec![-0.0, 0.0], 0.0),
    ] {
        for sql in ["SELECT Sum(Amount) FROM T", "SELECT DSum('Amount','T')"] {
            assert_eq!(
                total(&values, sql).unwrap().to_bits(),
                expected.to_bits(),
                "{sql}"
            );
        }
    }
    for values in [[f64::MAX, f64::MAX], [-f64::MAX, -f64::MAX]] {
        assert_eq!(
            total(&values, "SELECT Sum(Amount) FROM T"),
            Err(EvalError::Overflow.into())
        );
    }
    assert_eq!(
        total(
            &[1e308, 1.0, -1e308],
            "SELECT Sum(DSum('Amount','T')) FROM T"
        )
        .unwrap(),
        3.0
    );
    assert_eq!(
        total(
            &[1e308, 1.0, -1e308],
            "SELECT Sum(DAvg('Amount','T')) FROM T"
        )
        .unwrap(),
        1.0
    );
    let values = [1e308, 1.0, -1e308, 1e-300];
    for a in 0..4 {
        for b in 0..4 {
            for c in 0..4 {
                for d in 0..4 {
                    let order = [a, b, c, d];
                    if order
                        .iter()
                        .enumerate()
                        .any(|(i, n)| order[..i].contains(n))
                    {
                        continue;
                    }
                    let reordered: Vec<_> = order.iter().map(|index| values[*index]).collect();
                    assert_eq!(total(&reordered, "SELECT Sum(Amount) FROM T").unwrap(), 1.0);
                }
            }
        }
    }
}

#[test]
fn all_finite_exponent_bins_cancel_without_losing_the_residual() {
    let mut values: Vec<_> = (1_u64..=2046)
        .map(|exponent| f64::from_bits(exponent << 52))
        .collect();
    values.push(f64::from_bits(1));
    values.extend(
        (1_u64..=2046)
            .rev()
            .map(|exponent| -f64::from_bits(exponent << 52)),
    );
    assert_eq!(
        total(&values, "SELECT Sum(Amount) FROM T")
            .unwrap()
            .to_bits(),
        1
    );
}

#[test]
fn group_and_column_scratch_reuse_retains_nulls_means_and_limits() {
    let source = MemorySource::new(vec![
        MemoryTable::new(
            "T".into(),
            vec![
                Column::new("Bucket".into(), ScalarType::Int32, false).unwrap(),
                Column::new("Amount".into(), ScalarType::Double, true).unwrap(),
            ],
            (0..256)
                .map(|n| {
                    vec![
                        Value::Int32(n),
                        if n == 255 {
                            Value::Null
                        } else {
                            Value::Double(Double::from_float(f64::from(n) + 1.0))
                        },
                    ]
                })
                .collect(),
        )
        .unwrap(),
    ])
    .unwrap();
    let parsed = parse_query(
        "SELECT Bucket,Sum(Amount),Avg(Amount),Sum(Amount) FROM T GROUP BY Bucket ORDER BY Bucket",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let result = plan
        .execute(
            &source,
            &[],
            ExecutionOptions {
                limits: QueryLimits {
                    bytes: 1024 * 1024,
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(result.rows().len(), 256);
    for (index, row) in result.rows().iter().enumerate() {
        let expected = if index == 255 {
            Value::Null
        } else {
            Value::Double(Double::from_float(index as f64 + 1.0))
        };
        assert_eq!(&row[1..], &[expected.clone(), expected.clone(), expected]);
    }
    let exhausted = plan.execute(
        &source,
        &[],
        ExecutionOptions {
            limits: QueryLimits {
                bytes: 30000,
                ..Default::default()
            },
            ..Default::default()
        },
    );
    assert!(
        matches!(exhausted, Err(QueryError::Limit(error)) if error.resource == Resource::Bytes)
    );
}

#[test]
fn sum_checks_cancellation_and_evaluates_volatile_operands_once() {
    let source = source(&[1e308, 1.0, -1e308]);
    let parsed = parse_query("SELECT Sum(Amount) FROM T", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let calls = Cell::new(0);
    let count = || {
        calls.set(calls.get() + 1);
        false
    };
    plan.execute(
        &source,
        &[],
        ExecutionOptions {
            cancelled: Some(&count),
            ..Default::default()
        },
    )
    .unwrap();
    for stop in 1..=calls.get() {
        let calls = Cell::new(0);
        let cancelled = || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        };
        assert!(matches!(
            plan.execute(
                &source,
                &[],
                ExecutionOptions {
                    cancelled: Some(&cancelled),
                    ..Default::default()
                }
            ),
            Err(QueryError::Cancelled)
        ));
    }
    let random = RndState::new(123).unwrap();
    let reference = RndState::new(123).unwrap();
    let mut expected = 0.0;
    for _ in 0..3 {
        let Value::Single(value) = reference.apply(None).unwrap() else {
            panic!()
        };
        expected += f64::from(value.to_float());
    }
    let parsed = parse_query("SELECT Sum(Rnd()) FROM T", Default::default()).unwrap();
    let result = prepare(parsed.query(), &source)
        .unwrap()
        .execute(
            &source,
            &[],
            ExecutionOptions {
                random: Some(&random),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        result.rows(),
        &[vec![Value::Double(Double::from_float(expected))]]
    );
    assert_eq!(random.apply(None).unwrap(), reference.apply(None).unwrap());
}

#[test]
fn fixed_point_mixed_admission_and_late_operand_errors_remain_explicit() {
    let source = MemorySource::default();
    for (sql, error) in [
        (
            "SELECT Sum(IIf(N=1,CCur(1),CDbl(1))) FROM (SELECT 1 AS N UNION ALL SELECT 2) AS Q",
            EvalError::UnsupportedNumericCombination,
        ),
        (
            "SELECT Sum(IIf(N=1,CDbl(1),CCur(1))) FROM (SELECT 1 AS N UNION ALL SELECT 2) AS Q",
            EvalError::UnsupportedNumericCombination,
        ),
        (
            "SELECT Sum(IIf(N=1,CDbl(1),CLngLng(9007199254740993))) FROM (SELECT 1 AS N UNION ALL SELECT 2) AS Q",
            EvalError::PrecisionLoss,
        ),
        (
            "SELECT Sum(CLngLng(9223372036854775807)) FROM (SELECT 1 AS N UNION ALL SELECT 2) AS Q",
            EvalError::Overflow,
        ),
        (
            "SELECT Sum(IIf(N=1,CDbl(1),1/0)) FROM (SELECT 1 AS N UNION ALL SELECT 2) AS Q",
            EvalError::DivisionByZero,
        ),
        (
            "SELECT Sum(1/(N-1)) FROM (SELECT 2 AS N UNION ALL SELECT 1) AS Q",
            EvalError::DivisionByZero,
        ),
        (
            "SELECT Sum(IIf(N<3,1.7976931348623157e308,0.0)+1/(N-3)) FROM (SELECT 1 AS N UNION ALL SELECT 2 UNION ALL SELECT 3) AS Q",
            EvalError::DivisionByZero,
        ),
    ] {
        let parsed = parse_query(sql, Default::default()).unwrap();
        assert_eq!(
            prepare(parsed.query(), &source)
                .unwrap()
                .execute(&source, &[], Default::default())
                .err(),
            Some(error.into())
        );
    }
    let parsed = parse_query("SELECT Sum(IIf(N=1,CDbl(1.25),CDec(2.5))),Sum(NULL) FROM (SELECT 1 AS N UNION ALL SELECT 2) AS Q", Default::default()).unwrap();
    let result = prepare(parsed.query(), &source)
        .unwrap()
        .execute(&source, &[], Default::default())
        .unwrap();
    assert!(
        matches!(result.rows()[0][0], Value::Decimal(value) if value.coefficient() == 375 && value.scale() == 2)
    );
    assert_eq!(result.rows()[0][1], Value::Null);
}

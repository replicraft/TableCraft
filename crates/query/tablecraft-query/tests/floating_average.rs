use std::cell::Cell;
use tablecraft_core::{
    limits::Resource,
    values::{Double, Value},
};
use tablecraft_expressions::functions::math::random::RndState;
use tablecraft_query::{
    ExecutionOptions, QueryError, QueryLimits, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::{ParseLimits, parse_query};

fn source(values: &[f64]) -> MemorySource {
    MemorySource::new(vec![
        MemoryTable::new(
            "T".into(),
            vec![Column::new("Amount".into(), ScalarType::Double, false).unwrap()],
            values
                .iter()
                .map(|value| vec![Value::Double(Double::from_float(*value))])
                .collect(),
        )
        .unwrap(),
    ])
    .unwrap()
}
fn mean(values: &[f64]) -> f64 {
    query_mean(values, "SELECT Avg(Amount) FROM T")
}
fn query_mean(values: &[f64], sql: &str) -> f64 {
    let source = source(values);
    let parsed = parse_query(sql, ParseLimits::default()).unwrap();
    let result = prepare(parsed.query(), &source)
        .unwrap()
        .execute(&source, &[], ExecutionOptions::default())
        .unwrap();
    let Value::Double(value) = result.rows()[0][0] else {
        panic!("floating mean lost its carrier");
    };
    value.to_float()
}
#[test]
fn finite_means_survive_large_totals_cancellation_subnormals_and_ties() {
    for (values, expected) in [
        (vec![f64::MAX, f64::MAX], f64::MAX),
        (vec![-f64::MAX, -f64::MAX], -f64::MAX),
        (vec![f64::MAX, 1.0, -f64::MAX], 1.0 / 3.0),
        (vec![1e308, 1e-300, -1e308], 1e-300 / 3.0),
        (
            vec![f64::from_bits(1), f64::from_bits(1)],
            f64::from_bits(1),
        ),
        (vec![f64::from_bits(1), 0.0], 0.0),
        (
            vec![f64::from_bits(1), f64::from_bits(2)],
            f64::from_bits(2),
        ),
        (vec![f64::MAX, -f64::MAX], 0.0),
    ] {
        assert_eq!(mean(&values).to_bits(), expected.to_bits());
    }
}
#[test]
fn floating_input_order_does_not_change_the_exact_binary_mean() {
    let values = [1e308, 1.0, -1e308, 1e-300];
    let expected: f64 = (1.0 + 1e-300) / 4.0;
    for a in 0..4 {
        for b in 0..4 {
            for c in 0..4 {
                for d in 0..4 {
                    if [a, b, c, d]
                        .iter()
                        .enumerate()
                        .any(|(index, value)| [a, b, c, d][..index].contains(value))
                    {
                        continue;
                    }
                    assert_eq!(
                        mean(&[values[a], values[b], values[c], values[d]]).to_bits(),
                        expected.to_bits()
                    );
                }
            }
        }
    }
}
#[test]
fn reused_group_scratch_has_no_previous_exponents_or_coefficients() {
    let rows = [
        (1, f64::MAX),
        (1, f64::MAX),
        (2, 1.0),
        (2, 2.0),
        (3, 1e308),
        (3, 1e-300),
        (3, -1e308),
    ]
    .into_iter()
    .map(|(group, value)| {
        vec![
            Value::Int32(group),
            Value::Double(Double::from_float(value)),
        ]
    })
    .collect();
    let source = MemorySource::new(vec![
        MemoryTable::new(
            "T".into(),
            vec![
                Column::new("Bucket".into(), ScalarType::Int32, false).unwrap(),
                Column::new("Amount".into(), ScalarType::Double, false).unwrap(),
            ],
            rows,
        )
        .unwrap(),
    ])
    .unwrap();
    let parsed = parse_query(
        "SELECT Bucket,Avg(Amount) FROM T GROUP BY Bucket ORDER BY Bucket",
        ParseLimits::default(),
    )
    .unwrap();
    let result = prepare(parsed.query(), &source)
        .unwrap()
        .execute(&source, &[], ExecutionOptions::default())
        .unwrap();
    for (row, expected) in result.rows().iter().zip([f64::MAX, 1.5, 1e-300 / 3.0]) {
        let Value::Double(value) = row[1] else {
            panic!("missing Double mean");
        };
        assert_eq!(value.to_float().to_bits(), expected.to_bits());
    }
}
#[test]
fn scratch_and_integer_conversion_work_remain_metered_and_cancellable() {
    let source = source(&[f64::MAX, 1.0, -f64::MAX]);
    let parsed = parse_query("SELECT Avg(Amount) FROM T", ParseLimits::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let calls = Cell::new(0);
    let cancelled = || {
        calls.set(calls.get() + 1);
        false
    };
    plan.execute(
        &source,
        &[],
        ExecutionOptions {
            cancelled: Some(&cancelled),
            ..Default::default()
        },
    )
    .unwrap();
    let total = calls.get();
    for stop in 1..=total {
        calls.set(0);
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
    assert!(
        matches!(plan.execute(&source,&[],ExecutionOptions { limits:QueryLimits { bytes:30000,..Default::default() },..Default::default() }),Err(QueryError::Limit(error)) if error.resource == Resource::Bytes)
    );
}

#[test]
fn cancellation_across_the_full_finite_exponent_range_keeps_the_residual() {
    let mut values = Vec::new();
    for exponent in 1_u64..=2046 {
        let value = f64::from_bits(exponent << 52);
        values.push(value);
    }
    values.push(1.0);
    for exponent in (1_u64..=2046).rev() {
        values.push(-f64::from_bits(exponent << 52));
    }
    assert_eq!(
        mean(&values).to_bits(),
        (1.0 / values.len() as f64).to_bits()
    );
}

#[test]
fn singleton_groups_reuse_bounded_scratch_with_a_small_shared_budget() {
    let source = MemorySource::new(vec![
        MemoryTable::new(
            "T".into(),
            vec![
                Column::new("Bucket".into(), ScalarType::Int32, false).unwrap(),
                Column::new("Amount".into(), ScalarType::Double, false).unwrap(),
            ],
            (0..256)
                .map(|n| {
                    vec![
                        Value::Int32(n),
                        Value::Double(Double::from_float(f64::from(n) + 1.0)),
                    ]
                })
                .collect(),
        )
        .unwrap(),
    ])
    .unwrap();
    let parsed = parse_query(
        "SELECT Bucket,Avg(Amount) FROM T GROUP BY Bucket ORDER BY Bucket",
        ParseLimits::default(),
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
    for (n, row) in result.rows().iter().enumerate() {
        assert_eq!(row[1], Value::Double(Double::from_float(n as f64 + 1.0)));
    }
    let calls = Cell::new(0);
    let cancelled = || {
        calls.set(calls.get() + 1);
        calls.get() == 100
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

#[test]
fn exact_mean_evaluates_volatile_arguments_only_once_per_input() {
    let source = source(&[1.0, 2.0, 3.0]);
    let state = RndState::new(123).unwrap();
    let reference = RndState::new(123).unwrap();
    let mut total = 0.0;
    for _ in 0..3 {
        let Value::Single(value) = reference.apply(None).unwrap() else {
            panic!("unexpected Rnd carrier");
        };
        total += f64::from(value.to_float());
    }
    let parsed = parse_query("SELECT Avg(Rnd()) FROM T", ParseLimits::default()).unwrap();
    let result = prepare(parsed.query(), &source)
        .unwrap()
        .execute(
            &source,
            &[],
            ExecutionOptions {
                random: Some(&state),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        result.rows(),
        [vec![Value::Double(Double::from_float(total / 3.0))]]
    );
    assert_eq!(state.apply(None).unwrap(), reference.apply(None).unwrap());
}

#[test]
fn domain_means_reuse_exact_float_accumulation() {
    assert_eq!(
        query_mean(&[f64::MAX, f64::MAX], "SELECT DAvg('Amount','T')").to_bits(),
        f64::MAX.to_bits()
    );
    assert_eq!(
        query_mean(&[1e308, 1e-300, -1e308], "SELECT DAvg('Amount','T')").to_bits(),
        (1e-300_f64 / 3.0).to_bits()
    );
}

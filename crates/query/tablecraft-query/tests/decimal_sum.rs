//! Wider Decimal sums retain residuals; ordinary prefix families keep their policy.
use std::{
    cell::Cell,
    sync::atomic::{AtomicUsize, Ordering},
};
use tablecraft_core::{
    limits::Resource,
    values::{Currency, Decimal, ExtendedDateTime, Value},
};
use tablecraft_expressions::{
    EvalError,
    functions::{
        conversions::text::NumberFormat,
        dates::clock::{Clock, ClockReading},
    },
};
use tablecraft_query::{
    ExecutionOptions, QueryError, QueryLimits, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::parse_query;

const MAXIMUM: i128 = 79228162514264337593543950335;

fn decimal(coefficient: i128, scale: u8) -> Value {
    Value::Decimal(Decimal::new(coefficient, scale).unwrap())
}

fn source(values: Vec<Value>) -> MemorySource {
    MemorySource::new(vec![
        MemoryTable::new(
            "T".into(),
            vec![
                Column::new("X".into(), ScalarType::Decimal, true).unwrap(),
                Column::new("ID".into(), ScalarType::Int32, false).unwrap(),
            ],
            values
                .into_iter()
                .enumerate()
                .map(|(index, value)| vec![value, Value::Int32(i32::try_from(index).unwrap())])
                .collect(),
        )
        .unwrap(),
    ])
    .unwrap()
}

fn run(sql: &str, data: &MemorySource) -> Result<Vec<Vec<Value>>, QueryError> {
    let parsed = parse_query(sql, Default::default()).unwrap();
    Ok(prepare(parsed.query(), data)?
        .execute(data, &[], Default::default())?
        .rows()
        .to_vec())
}

#[test]
fn wide_decimal_sums_keep_residuals_and_cancel_intermediate_overflow_in_every_order() {
    for (values, expected) in [
        (
            vec![decimal(MAXIMUM, 0), decimal(1, 28), decimal(-MAXIMUM, 0)],
            decimal(1, 28),
        ),
        (
            vec![decimal(MAXIMUM, 0), decimal(-1, 28), decimal(-MAXIMUM, 0)],
            decimal(-1, 28),
        ),
        (
            vec![
                decimal(MAXIMUM, 0),
                decimal(MAXIMUM, 0),
                decimal(-MAXIMUM, 0),
            ],
            decimal(MAXIMUM, 0),
        ),
    ] {
        for order in [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ] {
            let data = source(
                order
                    .into_iter()
                    .map(|index| values[index].clone())
                    .collect(),
            );
            for sql in ["SELECT Sum(X) FROM T", "SELECT DSum('X','T')"] {
                assert_eq!(run(sql, &data).unwrap(), [vec![expected.clone()]], "{sql}");
            }
        }
    }
}

#[test]
fn final_decimal_rounding_uses_highest_fitting_scale_and_half_even() {
    for sign in [1, -1] {
        for base in [MAXIMUM - 1, MAXIMUM - 2] {
            let data = source(vec![decimal(sign * base, 1), decimal(sign * 5, 2)]);
            assert_eq!(
                run("SELECT Sum(X) FROM T", &data).unwrap(),
                [vec![decimal(sign * (MAXIMUM - 1), 1)]]
            );
        }
    }
    let data = source(vec![decimal(MAXIMUM, 0), decimal(MAXIMUM, 0)]);
    assert_eq!(
        run("SELECT Sum(X) FROM T", &data),
        Err(QueryError::Evaluation(EvalError::Overflow))
    );
}

#[test]
fn empty_null_grouped_derived_and_crosstab_paths_keep_the_same_sum_state() {
    let data = source(vec![
        decimal(MAXIMUM, 0),
        decimal(1, 28),
        decimal(-MAXIMUM, 0),
        Value::Null,
    ]);
    for sql in [
        "SELECT Sum(X) FROM T",
        "SELECT Sum(D.X) FROM (SELECT X FROM T) AS D",
        "SELECT Sum(X) FROM T GROUP BY 1+1",
        "SELECT Sum(X) FROM T GROUP BY 1+1 HAVING Sum(X)>0",
    ] {
        assert_eq!(run(sql, &data).unwrap(), [vec![decimal(1, 28)]], "{sql}");
    }
    assert_eq!(
        run("SELECT Sum(X) FROM T WHERE False", &data).unwrap(),
        [vec![Value::Null]]
    );
    assert_eq!(
        run("SELECT Sum(X) FROM T WHERE ID=3", &data).unwrap(),
        [vec![Value::Null]]
    );
    assert_eq!(
        run(
            "TRANSFORM Sum(X) SELECT 1 AS K FROM T GROUP BY 1 PIVOT 1 IN (1,2)",
            &data
        )
        .unwrap(),
        [vec![Value::Int64(1), decimal(1, 28), Value::Null]]
    );
}

#[test]
fn late_operand_errors_precede_final_range_checks_and_plain_prefix_overflow_stays_checked() {
    let data = source(vec![
        decimal(MAXIMUM, 0),
        decimal(MAXIMUM, 0),
        decimal(1, 0),
    ]);
    assert_eq!(
        run("SELECT Sum(IIf(ID=2,1/(2-ID),X)) FROM T", &data),
        Err(QueryError::Evaluation(EvalError::DivisionByZero))
    );
    assert_eq!(
        run("SELECT Sum(IIf(ID=2,CVErr(7),X)) FROM T", &data),
        Err(QueryError::Evaluation(EvalError::TypeMismatch))
    );
    for (kind, values) in [
        (
            ScalarType::Int64,
            vec![Value::Int64(i64::MAX), Value::Int64(1)],
        ),
        (
            ScalarType::Currency,
            vec![
                Value::Currency(Currency::from_scaled(i64::MAX)),
                Value::Currency(Currency::from_scaled(1)),
            ],
        ),
    ] {
        let data = MemorySource::new(vec![
            MemoryTable::new(
                "P".into(),
                vec![Column::new("X".into(), kind, false).unwrap()],
                values.into_iter().map(|value| vec![value]).collect(),
            )
            .unwrap(),
        ])
        .unwrap();
        assert_eq!(
            run("SELECT Sum(X) FROM P", &data),
            Err(QueryError::Evaluation(EvalError::Overflow))
        );
    }
}

#[test]
fn wide_sum_work_allocation_and_every_cancellation_checkpoint_are_observable() {
    let data = source(vec![
        decimal(MAXIMUM, 0),
        decimal(1, 28),
        decimal(-MAXIMUM, 0),
    ]);
    let parsed = parse_query("SELECT Sum(X) FROM T", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &data).unwrap();
    let calls = Cell::new(0);
    plan.execute(
        &data,
        &[],
        ExecutionOptions {
            cancelled: Some(&|| {
                calls.set(calls.get() + 1);
                false
            }),
            ..Default::default()
        },
    )
    .unwrap();
    let total = calls.get();
    for stop in 1..=total {
        calls.set(0);
        assert!(matches!(
            plan.execute(
                &data,
                &[],
                ExecutionOptions {
                    cancelled: Some(&|| {
                        calls.set(calls.get() + 1);
                        calls.get() == stop
                    }),
                    ..Default::default()
                }
            ),
            Err(QueryError::Cancelled)
        ));
    }
    assert!(matches!(plan.execute(&data, &[], ExecutionOptions {
        limits: QueryLimits { bytes: 1, ..Default::default() },
        ..Default::default()
    }), Err(QueryError::Limit(error)) if error.resource == Resource::Bytes));
}

#[test]
fn exact_decimal_retention_never_reevaluates_volatile_operands() {
    struct Counting(AtomicUsize);
    impl Clock for Counting {
        fn read(&self) -> Result<ClockReading, EvalError> {
            let second = u8::try_from(self.0.fetch_add(1, Ordering::SeqCst)).unwrap();
            ClockReading::new(ExtendedDateTime::new(2026, 10, 24, 0, 0, second, 0).unwrap())
        }
    }

    let data = source(vec![
        decimal(MAXIMUM, 0),
        decimal(0, 0),
        decimal(-MAXIMUM, 0),
    ]);
    let clock = Counting(AtomicUsize::new(0));
    let parsed = parse_query(
        "SELECT Sum(IIf(ID=1,CDec(Second(Now()))/CDec('10000000000000000000000000000'),X)) FROM T",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &data).unwrap();
    assert_eq!(clock.0.load(Ordering::SeqCst), 0);
    let result = plan
        .execute(
            &data,
            &[],
            ExecutionOptions {
                clock: Some(&clock),
                number_format: Some(NumberFormat::EnUs),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(result.rows(), [vec![decimal(1, 28)]]);
    assert_eq!(clock.0.load(Ordering::SeqCst), 3);
}

#[test]
fn final_rounding_shares_work_and_cancellation_instead_of_publishing_a_partial_sum() {
    let data = source(vec![decimal(MAXIMUM - 1, 1), decimal(5, 2)]);
    let parsed = parse_query("SELECT Sum(X) FROM T", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &data).unwrap();
    let calls = Cell::new(0);
    plan.execute(
        &data,
        &[],
        ExecutionOptions {
            cancelled: Some(&|| {
                calls.set(calls.get() + 1);
                false
            }),
            ..Default::default()
        },
    )
    .unwrap();
    let total = calls.get();
    for stop in 1..=total {
        calls.set(0);
        assert!(matches!(
            plan.execute(
                &data,
                &[],
                ExecutionOptions {
                    cancelled: Some(&|| {
                        calls.set(calls.get() + 1);
                        calls.get() == stop
                    }),
                    ..Default::default()
                }
            ),
            Err(QueryError::Cancelled)
        ));
    }
}

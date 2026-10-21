use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use tablecraft_core::values::ExtendedDateTime;
use tablecraft_expressions::functions::dates::clock::{Clock, ClockReading};
use tablecraft_query::QueryLimits;

#[test]
fn repeated_centered_passes_do_not_reevaluate_volatile_arguments() {
    struct Counting(AtomicUsize);
    impl Clock for Counting {
        fn read(&self) -> Result<ClockReading, EvalError> {
            ClockReading::new(
                ExtendedDateTime::new(
                    2000,
                    1,
                    1,
                    0,
                    0,
                    self.0.fetch_add(1, Ordering::Relaxed) as u8,
                    0,
                )
                .unwrap(),
            )
        }
    }
    let data = ints(&[Some(1), Some(2), Some(3)]);
    let clock = Counting(AtomicUsize::new(0));
    let parsed = parse_query("SELECT Var(Second(Now())) FROM T", ParseLimits::default()).unwrap();
    let plan = prepare(parsed.query(), &data).unwrap();
    assert_eq!(clock.0.load(Ordering::Relaxed), 0);
    let result = plan
        .execute(
            &data,
            &[],
            ExecutionOptions {
                clock: Some(&clock),
                ..Default::default()
            },
        )
        .unwrap();
    near(float(&result.rows()[0][0]), 1.0);
    assert_eq!(clock.0.load(Ordering::Relaxed), 3);
}
#[test]
fn every_observed_statistical_checkpoint_can_cancel_and_quotas_remain_cumulative() {
    let data = ints(&[Some(1), Some(2), Some(3), None]);
    let parsed = parse_query("SELECT Var(X),StDevP(X) FROM T", ParseLimits::default()).unwrap();
    let plan = prepare(parsed.query(), &data).unwrap();
    let calls = std::cell::Cell::new(0);
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
    for stop in 1..=calls.get() {
        let checks = std::cell::Cell::new(0);
        assert_eq!(
            plan.execute(
                &data,
                &[],
                ExecutionOptions {
                    cancelled: Some(&|| {
                        checks.set(checks.get() + 1);
                        checks.get() == stop
                    }),
                    ..Default::default()
                }
            )
            .err(),
            Some(QueryError::Cancelled)
        );
    }
    for limits in [
        QueryLimits {
            bytes: 1,
            ..Default::default()
        },
        QueryLimits {
            steps: 15,
            ..Default::default()
        },
    ] {
        assert!(
            plan.execute(
                &data,
                &[],
                ExecutionOptions {
                    limits,
                    ..Default::default()
                }
            )
            .is_err()
        );
    }
}

#[test]
fn statistical_value_buffer_is_charged_beyond_the_existing_group_storage() {
    let data = source(
        ScalarType::Double,
        (0..20)
            .map(|i| Value::Double(Double::from_float(f64::from(i))))
            .collect(),
    );
    let parse = |sql| parse_query(sql, ParseLimits::default()).unwrap();
    let min = parse("SELECT Min(X) FROM T");
    let min = prepare(min.query(), &data).unwrap();
    let var = parse("SELECT Var(X) FROM T");
    let var = prepare(var.query(), &data).unwrap();
    let mut low = 0;
    let mut high = QueryLimits::MAX_BYTES;
    while low < high {
        let middle = (low + high) / 2;
        let result = min.execute(
            &data,
            &[],
            ExecutionOptions {
                limits: QueryLimits {
                    bytes: middle,
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        if result.is_ok() {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    assert!(
        min.execute(
            &data,
            &[],
            ExecutionOptions {
                limits: QueryLimits {
                    bytes: low,
                    ..Default::default()
                },
                ..Default::default()
            }
        )
        .is_ok()
    );
    assert!(
        var.execute(
            &data,
            &[],
            ExecutionOptions {
                limits: QueryLimits {
                    bytes: low,
                    ..Default::default()
                },
                ..Default::default()
            }
        )
        .is_err()
    );
}

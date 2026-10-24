use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use tablecraft_core::values::ExtendedDateTime;
use tablecraft_expressions::functions::dates::clock::{Clock, ClockReading};
use tablecraft_query::QueryLimits;

#[test]
fn date_mean_work_is_cancellable_and_numeric_scratch_stays_budgeted() {
    let source = Dates::new(vec![date(-1.25), Value::Null, date(2.75)]);
    let parsed = parse_query(
        "SELECT Avg(Moment),DAvg('Moment','T') FROM T",
        Default::default(),
    )
    .unwrap();
    let prepared = prepare(parsed.query(), &source).unwrap();
    let checks = Cell::new(0);
    prepared
        .execute(
            &source,
            &[],
            ExecutionOptions {
                cancelled: Some(&|| {
                    checks.set(checks.get() + 1);
                    false
                }),
                ..Default::default()
            },
        )
        .unwrap();

    for stop in 1..=checks.get() {
        let observed = Cell::new(0);
        let cancelled = || {
            observed.set(observed.get() + 1);
            observed.get() >= stop
        };
        assert!(matches!(
            prepared.execute(
                &source,
                &[],
                ExecutionOptions {
                    cancelled: Some(&cancelled),
                    ..Default::default()
                }
            ),
            Err(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
        ));
    }

    for limits in [
        QueryLimits {
            bytes: 0,
            ..Default::default()
        },
        QueryLimits {
            steps: 0,
            ..Default::default()
        },
    ] {
        assert!(
            prepared
                .execute(
                    &source,
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
fn averaging_clock_dates_evaluates_each_argument_once_and_rejects_extended_parameters() {
    struct CountingClock(AtomicUsize);

    impl Clock for CountingClock {
        fn read(&self) -> Result<ClockReading, EvalError> {
            let second = self.0.fetch_add(1, Ordering::Relaxed) as u8;
            ClockReading::new(ExtendedDateTime::new(2000, 1, 1, 0, 0, second, 0).unwrap())
        }
    }

    let source = Dates::new(vec![date(1.0), date(2.0), Value::Null]);
    let parsed = parse_query("SELECT Avg(Now()) FROM T", Default::default()).unwrap();
    let clock = CountingClock(AtomicUsize::new(0));
    let prepared = prepare(parsed.query(), &source).unwrap();
    assert_eq!(clock.0.load(Ordering::Relaxed), 0);
    let result = prepared
        .execute(
            &source,
            &[],
            ExecutionOptions {
                clock: Some(&clock),
                ..Default::default()
            },
        )
        .unwrap();
    assert!(matches!(result.rows()[0][0], Value::Double(_)));
    assert_eq!(clock.0.load(Ordering::Relaxed), 3);

    let parsed = parse_query(
        "PARAMETERS P DATETIMEEXTENDED; SELECT Avg(P)",
        Default::default(),
    )
    .unwrap();
    let prepared = prepare(parsed.query(), &source).unwrap();
    let value = Value::DateTimeExtended(ExtendedDateTime::new(2024, 1, 1, 0, 0, 0, 0).unwrap());
    assert_eq!(
        prepared
            .execute(&source, &[("P", value)], Default::default())
            .err(),
        Some(EvalError::TypeMismatch.into())
    );
}

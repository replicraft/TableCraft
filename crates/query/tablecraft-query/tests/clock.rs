use std::sync::atomic::{AtomicUsize, Ordering};
use tablecraft_core::values::{ExtendedDateTime, LegacyDateTime, Single, Value};
use tablecraft_expressions::{
    EvalError,
    functions::dates::clock::{Clock, ClockReading},
};
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::{ParseLimits, parse_query};
struct CountingClock {
    calls: AtomicUsize,
    fail: bool,
}
impl Clock for CountingClock {
    fn read(&self) -> Result<ClockReading, EvalError> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        if self.fail {
            return Err(EvalError::ClockUnavailable);
        }
        ClockReading::new(ExtendedDateTime::new(2026, 10, 20, 12, 0, call as u8, 0).unwrap())
    }
}
fn date(value: &str) -> Value {
    Value::DateTime(LegacyDateTime::from_calendar(value.parse().unwrap()).unwrap())
}
#[test]
fn preparation_never_reads_clocks_and_execution_calls_are_not_cached() {
    let source = MemorySource::default();
    let clock = CountingClock {
        calls: AtomicUsize::new(0),
        fail: false,
    };
    let query = parse_query("SELECT Now(),Now(),(SELECT Now())", ParseLimits::default()).unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    assert_eq!(clock.calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                clock: Some(&clock),
                ..Default::default()
            }
        )
        .unwrap()
        .rows(),
        [vec![
            date("2026-10-20T12:00:00"),
            date("2026-10-20T12:00:01"),
            date("2026-10-20T12:00:02")
        ]]
    );
    assert_eq!(clock.calls.load(Ordering::SeqCst), 3);
    assert_eq!(
        plan.execute(&source, &[], ExecutionOptions::default())
            .err(),
        Some(QueryError::Evaluation(EvalError::ClockRequired))
    );
}
#[test]
fn clock_failures_remain_eager_even_in_unused_computed_projections() {
    let source = MemorySource::default();
    for sql in [
        "SELECT IIf(False,Now(),0)",
        "SELECT IIf(False,Timer(),0)",
        "SELECT A.Id FROM (SELECT 1 AS Id,Timer() AS Unused) AS A",
        "SELECT A.Id FROM (SELECT 1 AS Id,Now() AS Unused) AS A",
    ] {
        let clock = CountingClock {
            calls: AtomicUsize::new(0),
            fail: true,
        };
        let query = parse_query(sql, ParseLimits::default()).unwrap();
        let plan = prepare(query.query(), &source).unwrap();
        assert_eq!(clock.calls.load(Ordering::SeqCst), 0);
        assert_eq!(
            plan.execute(
                &source,
                &[],
                ExecutionOptions {
                    clock: Some(&clock),
                    ..Default::default()
                }
            )
            .err(),
            Some(QueryError::Evaluation(EvalError::ClockUnavailable))
        );
        assert_eq!(clock.calls.load(Ordering::SeqCst), 1);
    }
}
#[test]
fn volatile_clock_group_keys_are_not_replaced_by_projection_calls() {
    let source = MemorySource::default();
    let clock = CountingClock {
        calls: AtomicUsize::new(0),
        fail: false,
    };
    let query = parse_query(
        "SELECT Now(),Count(*) FROM (SELECT 1 AS Id UNION ALL SELECT 2 AS Id) AS A GROUP BY Now()",
        ParseLimits::default(),
    )
    .unwrap();
    let result = prepare(query.query(), &source)
        .unwrap()
        .execute(
            &source,
            &[],
            ExecutionOptions {
                clock: Some(&clock),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(result.rows().len(), 2);
    assert!(result.rows().iter().all(|row| row[1] == Value::Int64(1)));
    assert_eq!(clock.calls.load(Ordering::SeqCst), 4);
}

#[test]
fn composite_clock_keys_preserve_volatile_projection_boundaries() {
    let source = MemorySource::default();
    let clock = CountingClock {
        calls: AtomicUsize::new(0),
        fail: false,
    };
    let query = parse_query("SELECT Count(*) FROM (SELECT 1 AS Id UNION ALL SELECT 2 AS Id) AS A GROUP BY DateValue(Now())",ParseLimits::default()).unwrap();
    assert_eq!(
        prepare(query.query(), &source)
            .unwrap()
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    clock: Some(&clock),
                    ..Default::default()
                }
            )
            .unwrap()
            .rows(),
        [vec![Value::Int64(2)]]
    );
    assert_eq!(clock.calls.load(Ordering::SeqCst), 2);
    let query = parse_query(
        "SELECT A.D+Now() FROM (SELECT #2026-10-20# AS D) AS A GROUP BY A.D+Now()",
        ParseLimits::default(),
    )
    .unwrap();
    assert_eq!(
        prepare(query.query(), &source).err(),
        Some(QueryError::Grouping)
    );
}

#[test]
fn clock_names_remain_ordinary_field_names_without_parentheses() {
    let source = MemorySource::default();
    let query = parse_query(
        "SELECT A.[Date],A.[Time],A.[Now] FROM (SELECT 1 AS [Date],2 AS [Time],3 AS [Now]) AS A",
        ParseLimits::default(),
    )
    .unwrap();
    assert_eq!(
        prepare(query.query(), &source)
            .unwrap()
            .execute(&source, &[], ExecutionOptions::default())
            .unwrap()
            .rows(),
        [vec![Value::Int64(1), Value::Int64(2), Value::Int64(3)]]
    );
}

#[test]
fn timer_binding_is_inert_and_plan_reuse_reads_the_current_provider() {
    let source = MemorySource::default();
    let clock = CountingClock {
        calls: AtomicUsize::new(0),
        fail: false,
    };
    let query = parse_query("SELECT Timer(),(SELECT Timer())", ParseLimits::default()).unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    assert_eq!(clock.calls.load(Ordering::SeqCst), 0);

    for expected in [[43_200.0_f32, 43_201.0_f32], [43_202.0_f32, 43_203.0_f32]] {
        let result = plan
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    clock: Some(&clock),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(
            result.rows(),
            [expected
                .map(|value| Value::Single(Single::from_float(value)))
                .to_vec()]
        );
    }

    assert_eq!(clock.calls.load(Ordering::SeqCst), 4);
    assert_eq!(
        plan.execute(&source, &[], ExecutionOptions::default())
            .err(),
        Some(QueryError::Evaluation(EvalError::ClockRequired))
    );

    let query = parse_query(
        "SELECT A.[Timer] FROM (SELECT 7 AS [Timer]) AS A",
        ParseLimits::default(),
    )
    .unwrap();
    assert_eq!(
        prepare(query.query(), &source)
            .unwrap()
            .execute(&source, &[], ExecutionOptions::default())
            .unwrap()
            .rows(),
        [vec![Value::Int64(7)]]
    );
}

#[test]
fn timer_group_keys_retain_independent_volatile_projection_calls() {
    let source = MemorySource::default();
    let clock = CountingClock {
        calls: AtomicUsize::new(0),
        fail: false,
    };
    let query = parse_query(
        "SELECT Timer(),Count(*) FROM (SELECT 1 AS Id UNION ALL SELECT 2 AS Id) AS A GROUP BY Timer()",
        ParseLimits::default(),
    )
    .unwrap();
    let result = prepare(query.query(), &source)
        .unwrap()
        .execute(
            &source,
            &[],
            ExecutionOptions {
                clock: Some(&clock),
                ..Default::default()
            },
        )
        .unwrap();

    assert_eq!(result.rows().len(), 2);
    assert!(
        result
            .rows()
            .iter()
            .all(|row| matches!(row[0], Value::Single(_)) && row[1] == Value::Int64(1))
    );
    assert_eq!(clock.calls.load(Ordering::SeqCst), 4);
}

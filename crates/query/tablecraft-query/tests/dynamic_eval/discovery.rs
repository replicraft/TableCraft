use super::source;
use std::{
    cell::Cell,
    sync::atomic::{AtomicUsize, Ordering},
};
use tablecraft_core::values::{ExpressionValue, Value};
use tablecraft_expressions::{
    EvalError,
    functions::{
        conversions::text::NumberFormat,
        dates::clock::{Clock, ClockReading},
        math::random::RndState,
    },
};
use tablecraft_query::{
    ExecutionOptions, QueryError, QueryLimits, Source, execute_with_schema_discovery,
    source::MemorySource,
};
use tablecraft_schema::Column;
use tablecraft_sql::{parse_query, query::Query};

struct NamedSource {
    tables: MemorySource,
    queries: Vec<(&'static str, Query)>,
    scans: Cell<usize>,
}

impl NamedSource {
    fn new() -> Self {
        Self {
            tables: source(),
            queries: vec![(
                "Cross",
                parse_query(
                    "TRANSFORM First(Eval('Rnd()')) SELECT 1 AS G FROM Items GROUP BY 1 PIVOT ID",
                    Default::default(),
                )
                .unwrap()
                .into_query(),
            )],
            scans: Cell::new(0),
        }
    }
}

impl Source for NamedSource {
    fn columns(&self, name: &str) -> Result<&[Column], QueryError> {
        self.tables.columns(name)
    }

    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        Ok(self
            .queries
            .iter()
            .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
            .map(|(_, query)| query))
    }

    fn scan(
        &self,
        name: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.scans.set(self.scans.get() + 1);
        self.tables.scan(name, visitor)
    }
}

#[test]
fn key_initiated_discovery_does_not_reset_dynamic_repeatability() {
    let source = NamedSource::new();
    for sql in [
        "SELECT Count(*) FROM Items GROUP BY DLookup('[1]','Cross')",
        "TRANSFORM Count(*) SELECT 1 AS G FROM Items GROUP BY 1 PIVOT DLookup('[1]','Cross')",
    ] {
        let query = parse_query(sql, Default::default()).unwrap();
        let state = RndState::new(123).unwrap();
        let reference = RndState::new(123).unwrap();
        let result = execute_with_schema_discovery(
            query.query(),
            &source,
            &[],
            ExecutionOptions {
                random: Some(&state),
                number_format: Some(NumberFormat::Invariant),
                ..Default::default()
            },
        );
        assert_eq!(
            result.err(),
            Some(QueryError::Evaluation(EvalError::DynamicNotRepeatable)),
            "{sql}"
        );
        assert_eq!(state.apply(None).unwrap(), reference.apply(None).unwrap());
    }
}

#[test]
fn nested_key_discovery_inherits_context_through_named_schema_dependencies() {
    let mut source = NamedSource::new();
    source.queries.push((
        "OuterCross",
        parse_query(
            "TRANSFORM First([1]) SELECT 1 AS G FROM [Cross] GROUP BY 1 PIVOT G",
            Default::default(),
        )
        .unwrap()
        .into_query(),
    ));
    let query = parse_query(
        "SELECT Count(*) FROM Items GROUP BY DLookup('[1]','OuterCross')",
        Default::default(),
    )
    .unwrap();
    let state = RndState::new(123).unwrap();
    let reference = RndState::new(123).unwrap();
    assert_eq!(
        execute_with_schema_discovery(
            query.query(),
            &source,
            &[],
            ExecutionOptions {
                random: Some(&state),
                ..Default::default()
            },
        )
        .err(),
        Some(QueryError::Evaluation(EvalError::DynamicNotRepeatable))
    );
    assert_eq!(state.apply(None).unwrap(), reference.apply(None).unwrap());
}

#[test]
fn ordinary_discovery_keeps_complete_cached_values_without_duplicate_draws() {
    let source = NamedSource::new();
    let query = parse_query(
        "SELECT DLookup('[1]','Cross'),DLookup('[2]','Cross'),DLookup('[1]','Cross')",
        Default::default(),
    )
    .unwrap();
    let state = RndState::new(123).unwrap();
    let reference = RndState::new(123).unwrap();
    let first = reference.apply(None).unwrap();
    let second = reference.apply(None).unwrap();
    let result = execute_with_schema_discovery(
        query.query(),
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
        [vec![
            ExpressionValue::Value(first.clone()),
            ExpressionValue::Value(second),
            ExpressionValue::Value(first),
        ]]
    );
    assert_eq!(source.scans.get(), 1);
    assert_eq!(state.apply(None).unwrap(), reference.apply(None).unwrap());
}

#[test]
fn values_materialized_before_key_evaluation_remain_valid_group_inputs() {
    let source = NamedSource::new();
    for (sql, expected_count) in [
        (
            "SELECT Count(*) FROM Items WHERE DCount('*','Cross')=1 GROUP BY DLookup('[1]','Cross')",
            2,
        ),
        ("SELECT Count(*) FROM [Cross] GROUP BY [1]", 1),
    ] {
        let query = parse_query(sql, Default::default()).unwrap();
        let state = RndState::new(123).unwrap();
        let reference = RndState::new(123).unwrap();
        reference.apply(None).unwrap();
        reference.apply(None).unwrap();
        let result = execute_with_schema_discovery(
            query.query(),
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
            [vec![ExpressionValue::Value(Value::Int64(expected_count))]]
        );
        assert_eq!(state.apply(None).unwrap(), reference.apply(None).unwrap());
    }
}

struct RefusingClock(AtomicUsize);

impl Clock for RefusingClock {
    fn read(&self) -> Result<ClockReading, EvalError> {
        self.0.fetch_add(1, Ordering::Relaxed);
        Err(EvalError::ClockUnavailable)
    }
}

#[test]
fn inherited_repeatability_checks_clock_bodies_before_host_callbacks() {
    let mut source = NamedSource::new();
    source.queries[0].1 = parse_query(
        "TRANSFORM First(Eval('Now()')) SELECT 1 AS G FROM Items GROUP BY 1 PIVOT ID",
        Default::default(),
    )
    .unwrap()
    .into_query();
    let query = parse_query(
        "SELECT Count(*) FROM Items GROUP BY CVar(DLookup('[1]','Cross'))",
        Default::default(),
    )
    .unwrap();
    let clock = RefusingClock(AtomicUsize::new(0));
    assert_eq!(
        execute_with_schema_discovery(
            query.query(),
            &source,
            &[],
            ExecutionOptions {
                clock: Some(&clock),
                ..Default::default()
            },
        )
        .err(),
        Some(QueryError::Evaluation(EvalError::DynamicNotRepeatable))
    );
    assert_eq!(clock.0.load(Ordering::Relaxed), 0);
}

#[test]
fn a_failed_partial_discovery_does_not_survive_into_an_ordinary_execution() {
    let mut source = NamedSource::new();
    source.queries[0].1 = parse_query(
        "TRANSFORM First(Eval(IIf(ID=1,'1','Rnd()'))) SELECT 1 AS G FROM Items GROUP BY 1 PIVOT ID",
        Default::default(),
    )
    .unwrap()
    .into_query();
    let key = parse_query(
        "SELECT Count(*) FROM Items GROUP BY DLookup('[1]','Cross')",
        Default::default(),
    )
    .unwrap();
    let state = RndState::new(123).unwrap();
    let reference = RndState::new(123).unwrap();
    assert_eq!(
        execute_with_schema_discovery(
            key.query(),
            &source,
            &[],
            ExecutionOptions {
                random: Some(&state),
                ..Default::default()
            },
        )
        .err(),
        Some(QueryError::Evaluation(EvalError::DynamicNotRepeatable))
    );
    let scans_before = source.scans.get();
    let ordinary = parse_query(
        "SELECT DLookup('[1]','Cross'),DLookup('[2]','Cross')",
        Default::default(),
    )
    .unwrap();
    let result = execute_with_schema_discovery(
        ordinary.query(),
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
        [vec![
            ExpressionValue::Value(Value::Int64(1)),
            ExpressionValue::Value(reference.apply(None).unwrap()),
        ]]
    );
    assert_eq!(source.scans.get(), scans_before + 1);
    assert_eq!(state.apply(None).unwrap(), reference.apply(None).unwrap());
}

#[test]
fn inherited_discovery_context_keeps_cancellation_and_cumulative_quotas() {
    let mut source = NamedSource::new();
    source.queries[0].1 = parse_query(
        "TRANSFORM First(Eval('1+1')) SELECT 1 AS G FROM Items GROUP BY 1 PIVOT ID",
        Default::default(),
    )
    .unwrap()
    .into_query();
    let query = parse_query(
        "SELECT Count(*) FROM Items GROUP BY DLookup('[1]','Cross')",
        Default::default(),
    )
    .unwrap();
    let observed = Cell::new(0);
    let count = || {
        observed.set(observed.get() + 1);
        false
    };
    let result = execute_with_schema_discovery(
        query.query(),
        &source,
        &[],
        ExecutionOptions {
            cancelled: Some(&count),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        result.rows(),
        [vec![ExpressionValue::Value(Value::Int64(2))]]
    );
    assert_eq!(source.scans.get(), 2);

    for stop in 1..=observed.get() {
        let calls = Cell::new(0);
        let cancelled = || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        };
        assert!(
            matches!(
                execute_with_schema_discovery(
                    query.query(),
                    &source,
                    &[],
                    ExecutionOptions {
                        cancelled: Some(&cancelled),
                        ..Default::default()
                    },
                ),
                Err(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
            ),
            "checkpoint {stop}"
        );
    }
    for limits in [
        QueryLimits {
            bytes: 1,
            ..Default::default()
        },
        QueryLimits {
            steps: 1,
            ..Default::default()
        },
        QueryLimits {
            rows: 0,
            ..Default::default()
        },
    ] {
        assert!(matches!(
            execute_with_schema_discovery(
                query.query(),
                &source,
                &[],
                ExecutionOptions {
                    limits,
                    ..Default::default()
                },
            ),
            Err(QueryError::Limit(_) | QueryError::Evaluation(EvalError::Limit(_)))
        ));
    }
}

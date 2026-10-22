use tablecraft_core::values::Value;
use tablecraft_expressions::{EvalError, functions::math::random::RndState};
use tablecraft_query::{
    ExecutionOptions, PreparedQuery, QueryError, QueryLimits, prepare, source::MemorySource,
};
use tablecraft_sql::parse_query;

fn plan(sql: &str, source: &MemorySource) -> PreparedQuery {
    let parsed = parse_query(sql, Default::default()).unwrap();
    prepare(parsed.query(), source).unwrap()
}

#[test]
fn plan_reuse_does_not_capture_state_and_nested_calls_share_the_host_sequence() {
    let source = MemorySource::default();
    let prepared = plan("SELECT Rnd(),Rnd(0),(SELECT Rnd()),Rnd(0)", &source);
    let reference = RndState::default();
    let first = reference.apply(None).unwrap();
    let second = reference.apply(None).unwrap();
    let state = RndState::default();
    let result = prepared
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
        [vec![first.clone(), first, second.clone(), second]]
    );
    assert_eq!(
        prepared.execute(&source, &[], Default::default()).err(),
        Some(EvalError::RandomStateRequired.into())
    );
    let fresh = RndState::default();
    assert_eq!(
        prepared
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    random: Some(&fresh),
                    ..Default::default()
                }
            )
            .unwrap()
            .rows(),
        result.rows()
    );
    assert_ne!(
        prepared
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    random: Some(&state),
                    ..Default::default()
                }
            )
            .unwrap()
            .rows(),
        result.rows()
    );
}

#[test]
fn row_filters_aggregates_and_sort_aliases_preserve_observed_draws() {
    let source = MemorySource::default();
    let state = RndState::default();
    let reference = RndState::default();
    let prepared = plan(
        "SELECT A.N,Rnd(0) FROM (SELECT 1 AS N UNION ALL SELECT 2 UNION ALL SELECT 3) AS A WHERE A.N=1 AND Rnd()>0",
        &source,
    );
    let result = prepared
        .execute(
            &source,
            &[],
            ExecutionOptions {
                random: Some(&state),
                ..Default::default()
            },
        )
        .unwrap();
    // Filtering finishes for all input rows before projections execute.
    reference.apply(None).unwrap();
    reference.apply(None).unwrap();
    let last = reference.apply(None).unwrap();
    assert_eq!(result.rows(), [vec![Value::Int64(1), last]]);
    assert_eq!(
        state.apply(Some(&Value::Int16(0))),
        reference.apply(Some(&Value::Int16(0)))
    );
    let state = RndState::default();
    let prepared = plan(
        "SELECT Rnd() AS R FROM (SELECT 1 AS N UNION ALL SELECT 2 UNION ALL SELECT 3) AS A ORDER BY R",
        &source,
    );
    let result = prepared
        .execute(
            &source,
            &[],
            ExecutionOptions {
                random: Some(&state),
                ..Default::default()
            },
        )
        .unwrap();
    let draws = RndState::default();
    let mut expected = (0..3)
        .map(|_| {
            let Value::Single(value) = draws.apply(None).unwrap() else {
                panic!("Single required");
            };
            value.to_float()
        })
        .collect::<Vec<_>>();
    expected.sort_by(f32::total_cmp);
    for (row, expected) in result.rows().iter().zip(expected) {
        let Value::Single(actual) = row[0] else {
            panic!("Single required");
        };
        assert_eq!(actual.to_float(), expected);
    }
    assert_eq!(result.rows().len(), 3);
    assert_eq!(
        state.apply(Some(&Value::Int16(0))),
        reference.apply(Some(&Value::Int16(0)))
    );
    let state = RndState::default();
    let prepared = plan(
        "SELECT Sum(Rnd()) FROM (SELECT 1 AS N UNION ALL SELECT 2) AS A",
        &source,
    );
    assert_eq!(
        prepared
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    random: Some(&state),
                    ..Default::default()
                }
            )
            .unwrap()
            .rows()
            .len(),
        1
    );
    let parsed = parse_query("SELECT Rnd() GROUP BY Rnd()", Default::default()).unwrap();
    assert!(prepare(parsed.query(), &source).is_err());
}

#[test]
fn query_work_refusals_and_pre_cancelled_runs_do_not_mutate_state() {
    let source = MemorySource::default();
    let prepared = plan("SELECT Rnd()", &source);
    let state = RndState::default();
    assert_eq!(
        prepared
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    random: Some(&state),
                    cancelled: Some(&|| true),
                    ..Default::default()
                }
            )
            .err(),
        Some(QueryError::Cancelled)
    );
    let result = prepared.execute(
        &source,
        &[],
        ExecutionOptions {
            random: Some(&state),
            limits: QueryLimits {
                steps: 0,
                ..Default::default()
            },
            ..Default::default()
        },
    );
    assert!(matches!(result, Err(QueryError::Limit(_))));
    assert_eq!(state.apply(None), RndState::default().apply(None));
}

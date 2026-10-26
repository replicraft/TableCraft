use super::support::*;
use std::cell::Cell;
use tablecraft_core::values::Value;
use tablecraft_expressions::EvalError;
use tablecraft_query::{ExecutionOptions, QueryError, QueryLimits, prepare};

#[test]
fn selected_inner_errors_and_eof_failures_precede_outer_member_filtering_and_top() {
    let data = Data::fixture();
    for prefix in ["SELECT", "SELECT TOP 1"] {
        let sql = format!(
            "{prefix} Q.X.Value FROM (SELECT A AS X,1 \\ (Id-4) AS Boom FROM L) Q WHERE False"
        );
        assert_eq!(
            execute(&sql, &data, false).err(),
            Some(QueryError::Evaluation(EvalError::DivisionByZero))
        );
    }
    let plan = prepare(
        &syntax("SELECT Q.X.Value FROM (SELECT A AS X FROM L) Q"),
        &data,
    )
    .unwrap();
    data.fail_table.set(Some("L"));
    assert_eq!(
        plan.execute_collections(&data, &[], Default::default())
            .err(),
        Some(QueryError::Source)
    );
    data.fail_table.set(None);
    let result = plan
        .execute_collections(&data, &[], Default::default())
        .unwrap();
    assert_eq!(result.rows().len(), 6);
    assert!(!data.active.get());
}

#[test]
fn all_observed_cancellation_points_and_cumulative_pools_leave_the_plan_reusable() {
    let data = Data::fixture();
    let plan = prepare(
        &syntax("SELECT Q.X,Q.X.Value FROM (SELECT A AS X FROM L) Q"),
        &data,
    )
    .unwrap();
    let checkpoints = Cell::new(0);
    let observe = || {
        checkpoints.set(checkpoints.get() + 1);
        false
    };
    let expected = plan
        .execute_collections(
            &data,
            &[],
            ExecutionOptions {
                cancelled: Some(&observe),
                ..Default::default()
            },
        )
        .unwrap();
    for stop in 1..=checkpoints.get() {
        let calls = Cell::new(0);
        let cancel = || {
            calls.set(calls.get() + 1);
            calls.get() >= stop
        };
        let result = plan.execute_collections(
            &data,
            &[],
            ExecutionOptions {
                cancelled: Some(&cancel),
                ..Default::default()
            },
        );
        assert!(matches!(
            result.err(),
            Some(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
        ));
        assert!(!data.active.get());
    }
    for limits in [
        QueryLimits {
            bytes: 0,
            ..Default::default()
        },
        QueryLimits {
            rows: 0,
            ..Default::default()
        },
        QueryLimits {
            steps: 0,
            ..Default::default()
        },
    ] {
        let result = plan.execute_collections(
            &data,
            &[],
            ExecutionOptions {
                limits,
                ..Default::default()
            },
        );
        assert!(matches!(result.err(), Some(QueryError::Limit(_))));
    }
    data.swallow.set(true);
    let result = plan.execute_collections(
        &data,
        &[],
        ExecutionOptions {
            limits: QueryLimits {
                rows: 2,
                ..Default::default()
            },
            ..Default::default()
        },
    );
    assert!(matches!(result.err(), Some(QueryError::Limit(_))));
    assert!(data.rejected.get() > 0);
    data.swallow.set(false);
    let result = plan
        .execute_collections(&data, &[], Default::default())
        .unwrap();
    assert_eq!(result.rows(), expected.rows());
}

fn owned<'a>(value: i32) -> Data<'a> {
    let mut row = parent(1, &[Some(value)], &[Some(value + 1)]);
    row.scalars[1] = Value::Int32(value * 10);
    let mut data = Data::new(vec![Table::new("T", vec![row])]);
    data.definitions
        .push(("Owned", syntax("SELECT Id,A AS X FROM T")));
    data
}

#[test]
fn retained_named_owners_and_explicit_root_registry_are_distinct_from_leaf_identity() {
    let bridge = owned(701);
    let mut left = owned(501);
    left.definitions
        .push(("Pinned", syntax("SELECT Id,A AS X FROM [bridge].T")));
    let right = owned(601);
    let mut root = owned(999);
    root.external = vec![
        ("left", 1, &left),
        ("right", 2, &right),
        ("bridge", 3, &bridge),
    ];
    for (name, expected) in [("Owned", 501), ("Pinned", 701)] {
        let sql = format!(
            "SELECT P.X.Value,Q.X.Value FROM [left].{name} P \
             INNER JOIN [right].Owned Q ON P.Id=Q.Id"
        );
        let result = execute(&sql, &root, true).unwrap();
        assert_rows(&result, &[vec![scalar(expected), scalar(601)]]);
    }
    assert!(root.scans.borrow().is_empty());
    assert!(left.external_calls.borrow().is_empty());
    assert!(right.external_calls.borrow().is_empty());
    assert!(
        root.external_calls
            .borrow()
            .iter()
            .any(|token| token == "bridge")
    );

    // The external definition's lexical owner must not leak into a following
    // root-bound scalar subquery, even though all three Sources expose T.
    let sql = "SELECT P.X.Value,(SELECT K FROM T) AS RootKey FROM [left].Owned P";
    let result = execute(sql, &root, true).unwrap();
    assert_rows(&result, &[vec![scalar(501), scalar(9990)]]);
    assert_eq!(*root.scans.borrow(), ["T"]);
    assert_eq!(root.scalar_scans.get(), 1);
}

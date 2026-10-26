use super::support::*;
use std::cell::Cell;
use tablecraft_core::{limits::Resource, values::Value};
use tablecraft_expressions::EvalError;
use tablecraft_query::{ExecutionOptions, QueryError, QueryLimits, prepare};

#[test]
fn eager_displayed_errors_and_invalid_suffixes_never_publish_a_complete_prefix() {
    let data = Data::fixture();
    let sql = "SELECT TOP 1 A,10 \\ (A.Value-9) AS Boom FROM L";
    assert_eq!(
        execute(sql, &data, false).err(),
        Some(QueryError::Evaluation(EvalError::DivisionByZero))
    );

    let mut data = Data::fixture();
    data.tables[0].rows[0].lists[0].push(Value::Text("bad suffix".into()));
    let sql = "SELECT A,A.Value FROM L WHERE A.Value=7";
    assert_eq!(execute(sql, &data, false).err(), Some(QueryError::Source));

    let data = Data::fixture();
    data.fail_table.set(Some("L"));
    let plan = prepare(&syntax("SELECT A,A.Value FROM L"), &data).unwrap();
    assert_eq!(
        plan.execute_collections(&data, &[], Default::default())
            .err(),
        Some(QueryError::Source)
    );
    data.fail_table.set(None);
    let retried = plan
        .execute_collections(&data, &[], Default::default())
        .unwrap();
    assert_eq!(retried.rows().len(), 6);
}

#[test]
fn cumulative_product_limits_cancellation_and_ignored_rejections_remain_sticky() {
    let data = Data::fixture();
    let plan = prepare(&syntax("SELECT A,A.Value,B,B.Value FROM L"), &data).unwrap();
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
    assert_eq!(expected.rows().len(), 10);
    for checkpoint in 1..=checkpoints.get() {
        let calls = Cell::new(0);
        let cancel = || {
            calls.set(calls.get() + 1);
            calls.get() >= checkpoint
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
    for resource in [Resource::Bytes, Resource::Rows, Resource::Steps] {
        let mut limits = QueryLimits::default();
        match resource {
            Resource::Bytes => limits.bytes = 0,
            Resource::Rows => limits.rows = 0,
            Resource::Steps => limits.steps = 0,
        }
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
                rows: 3,
                ..Default::default()
            },
            ..Default::default()
        },
    );
    assert!(matches!(result.err(), Some(QueryError::Limit(_))));
    assert!(data.rejected.get() > 0);
    data.swallow.set(false);
    let retried = plan
        .execute_collections(&data, &[], Default::default())
        .unwrap();
    assert_eq!(retried.rows(), expected.rows());
}

#[test]
fn scalar_services_and_remaining_typed_shapes_keep_their_explicit_boundaries() {
    let data = Data::fixture();
    let plan = prepare(&syntax("SELECT A,A.Value FROM L"), &data).unwrap();
    assert_eq!(
        plan.execute(&data, &[], Default::default()).err(),
        Some(QueryError::Unsupported)
    );
    assert!(data.scans.borrow().is_empty());
    for (sql, error) in [
        (
            "SELECT DISTINCT A,A.Value FROM L WHERE False",
            QueryError::Unsupported,
        ),
        (
            "SELECT A,Count(A.Value) FROM L GROUP BY Id",
            QueryError::Unsupported,
        ),
        (
            "SELECT A,A.Value FROM L UNION ALL SELECT A,A.Value FROM R",
            QueryError::Unsupported,
        ),
        // Member access is supported, but typed DISTINCT remains unsupported.
        (
            "SELECT DISTINCT Q.A.Value,Q.A FROM (SELECT A FROM L) Q",
            QueryError::Unsupported,
        ),
    ] {
        let data = Data::fixture();
        assert_eq!(execute(sql, &data, false).err(), Some(error), "{sql}");
        assert!(data.scans.borrow().is_empty(), "{sql}");
    }
}

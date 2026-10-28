use super::*;
use tablecraft_query::{QueryLimits, prepare};

fn scalar(value: Value) -> QueryValue {
    QueryValue::Scalar(ExpressionValue::Value(value))
}

#[test]
fn outer_scalar_grouping_having_and_sort_use_complete_inner_parent_rows() {
    let data = Data::new();
    let result = execute(
        "SELECT D.Id,Count(*) AS N,Sum(D.Seq) AS Total \
         FROM (SELECT Id,Seq,Tags FROM LeftRows) D \
         WHERE D.Seq>=0 GROUP BY D.Id HAVING Count(*)>1 ORDER BY D.Id",
        &data,
    )
    .unwrap();
    assert_eq!(
        result.rows(),
        &[vec![
            scalar(Value::Int32(1)),
            scalar(Value::Int64(2)),
            scalar(Value::Int64(1))
        ]]
    );
    assert_eq!(*data.scans.borrow(), ["lists:LeftRows"]);
    assert_eq!(data.callbacks.get(), 3);
}

#[test]
fn named_scalar_totals_keep_empty_input_and_explicit_collection_execution() {
    let mut data = Data::new();
    let sql = "SELECT Count(*) AS N,Sum(Seq) AS Total FROM PairView";
    assert_eq!(
        execute(sql, &data).unwrap().rows(),
        &[vec![scalar(Value::Int64(3)), scalar(Value::Int64(3))]]
    );
    data.left.clear();
    data.left_lists.clear();
    assert_eq!(
        execute(sql, &data).unwrap().rows(),
        &[vec![scalar(Value::Int64(0)), scalar(Value::Null)]]
    );

    let syntax = parse_query(sql, Default::default()).unwrap();
    let plan = prepare(syntax.query(), &data).unwrap();
    assert_eq!(
        plan.execute(&data, &[], Default::default()).err(),
        Some(QueryError::Unsupported)
    );
}

#[test]
fn unused_inner_lists_and_scalar_projections_remain_eager_before_outer_filters() {
    let mut data = Data::new();
    data.right_lists[2].push(Value::Text("invalid hidden list suffix".into()));
    assert_eq!(
        execute(
            "SELECT Count(*) FROM (SELECT Id,Tags FROM RightRows) D WHERE False",
            &data,
        )
        .err(),
        Some(QueryError::Source)
    );
    data.right_lists[2].pop();
    data.fail_right_eof = true;
    assert_eq!(
        execute("SELECT Count(*) FROM (SELECT Tags FROM RightRows) D", &data).err(),
        Some(QueryError::Source)
    );

    let data = Data::new();
    assert_eq!(
        execute(
            "SELECT Count(*) FROM (SELECT Tags,1/(Id-1) AS Failure FROM LeftRows) D WHERE False",
            &data,
        )
        .err(),
        Some(QueryError::Evaluation(EvalError::DivisionByZero))
    );
}

#[test]
fn bare_list_and_outer_membership_domain_or_subquery_contexts_refuse_before_scans() {
    for sql in [
        "SELECT Count(Tags) FROM PairView",
        "SELECT Tags,Count(*) FROM PairView GROUP BY Tags",
        "SELECT DISTINCT Count(*) FROM PairView",
        "SELECT Count(*) FROM PairView WHERE Tags.Value=10",
        "SELECT Count(*) FROM PairView HAVING EXISTS(SELECT Id FROM Plain)",
        "SELECT Count(*),DCount('*','Plain') FROM PairView",
    ] {
        let data = Data::new();
        assert_eq!(
            execute(sql, &data).err(),
            Some(QueryError::Unsupported),
            "{sql}"
        );
        assert!(data.scans.borrow().is_empty(), "{sql}");
    }
}

#[test]
fn aggregate_boundary_keeps_sticky_limits_cancellation_and_reusable_plans() {
    let mut data = Data::new();
    data.ignore_failure = true;
    let syntax = parse_query("SELECT Count(*) FROM PairView", Default::default()).unwrap();
    let plan = prepare(syntax.query(), &data).unwrap();
    assert!(matches!(
        plan.execute_collections(
            &data,
            &[],
            ExecutionOptions {
                limits: QueryLimits {
                    rows: 1,
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .err(),
        Some(QueryError::Limit(_))
    ));
    let cancelled = || true;
    assert_eq!(
        plan.execute_collections(
            &data,
            &[],
            ExecutionOptions {
                cancelled: Some(&cancelled),
                ..Default::default()
            },
        )
        .err(),
        Some(QueryError::Cancelled)
    );
    assert_eq!(
        plan.execute_collections(&data, &[], Default::default())
            .unwrap()
            .rows(),
        &[vec![scalar(Value::Int64(3))]]
    );
}

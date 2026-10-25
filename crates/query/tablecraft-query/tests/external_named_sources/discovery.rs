use super::support::{Host, options, run};
use std::cell::Cell;
use tablecraft_core::values::{ExpressionValue, Value};
use tablecraft_query::{
    ExecutionOptions, QueryError, QueryLimits, execute_with_schema_discovery, prepare,
};
use tablecraft_sql::parse_query;

#[test]
fn fixed_crosstabs_with_equal_names_do_not_share_owner_rows() {
    let host = Host::new();
    let sql = "SELECT R.[10],A.[1],B.[2] FROM F R,(SELECT * FROM F IN 'one') A,(SELECT * FROM F IN 'two') B";
    assert_eq!(
        run(&host, sql).unwrap().rows(),
        &[vec![Value::Int64(1), Value::Int64(1), Value::Int64(1)]]
    );
    assert_eq!(host.root.scans.get(), 1);
    assert_eq!(host.one.scans.get(), 1);
    assert_eq!(host.two.scans.get(), 1);
}

#[test]
fn dynamic_discovery_and_reuse_are_separate_for_three_same_named_crosstabs() {
    let host = Host::new();
    let sql = "SELECT R.[10],A.[1],B.[2] FROM D R,(SELECT * FROM D IN 'one') A,(SELECT * FROM D IN 'two') B";
    let syntax = parse_query(sql, Default::default()).unwrap();
    assert!(prepare(syntax.query(), &host).is_err());
    assert_eq!(host.scans(), 0);

    let result = execute_with_schema_discovery(syntax.query(), &host, &[], options()).unwrap();
    assert_eq!(
        result.rows(),
        &[vec![
            ExpressionValue::Value(Value::Int64(1)),
            ExpressionValue::Value(Value::Int64(1)),
            ExpressionValue::Value(Value::Int64(1)),
        ]]
    );
    assert_eq!(host.root.scans.get(), 1);
    assert_eq!(host.one.scans.get(), 1);
    assert_eq!(host.two.scans.get(), 1);
}

#[test]
fn domains_can_discover_an_owner_crosstab_and_restore_the_root_sibling_owner() {
    let mut host = Host::new();
    host.one
        .define("Domain", "SELECT DSum('[1]','D') AS V FROM T");
    let syntax = parse_query(
        "SELECT A.V,DSum('Id','T') AS Root FROM (SELECT V FROM [Domain] IN 'one') A",
        Default::default(),
    )
    .unwrap();
    let result = execute_with_schema_discovery(syntax.query(), &host, &[], options()).unwrap();
    assert_eq!(
        result.rows(),
        &[vec![
            ExpressionValue::Value(Value::Int64(1)),
            ExpressionValue::Value(Value::Int64(10)),
        ]]
    );
    assert_eq!(host.one.scans.get(), 2);
    assert_eq!(host.root.scans.get(), 1);
}

#[test]
fn changed_external_revisions_fail_before_records_or_cached_results_are_published() {
    let host = Host::new();
    let syntax = parse_query(
        "SELECT R.Id,A.Id FROM Same R,(SELECT Id FROM Same IN 'one') A",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &host).unwrap();
    host.revision.set(17);
    assert!(matches!(
        plan.execute(&host, &[], options()),
        Err(QueryError::SchemaChanged)
    ));
    assert_eq!(host.scans(), 0);

    let host = Host::new();
    let syntax = parse_query("SELECT * FROM D IN 'one'", Default::default()).unwrap();
    host.one.change_revision.set(true);
    assert!(matches!(
        execute_with_schema_discovery(syntax.query(), &host, &[], options()),
        Err(QueryError::SchemaChanged)
    ));
    assert_eq!(host.one.scans.get(), 1);
}

#[test]
fn late_failures_cancellation_and_query_limits_do_not_publish_a_complete_prefix() {
    let host = Host::new();
    let syntax = parse_query("SELECT * FROM Nested IN 'one'", Default::default()).unwrap();
    let plan = prepare(syntax.query(), &host).unwrap();
    host.one.fail.set(true);
    assert!(matches!(
        plan.execute(&host, &[], options()),
        Err(QueryError::Source)
    ));
    host.one.fail.set(false);
    assert_eq!(plan.execute(&host, &[], options()).unwrap().rows().len(), 1);

    let calls = Cell::new(0);
    let count = || {
        calls.set(calls.get() + 1);
        false
    };
    plan.execute(
        &host,
        &[],
        ExecutionOptions {
            cancelled: Some(&count),
            ..options()
        },
    )
    .unwrap();
    for stop in 1..=calls.get() {
        let checks = Cell::new(0);
        let cancelled = || {
            checks.set(checks.get() + 1);
            checks.get() == stop
        };
        let result = plan.execute(
            &host,
            &[],
            ExecutionOptions {
                cancelled: Some(&cancelled),
                ..options()
            },
        );
        assert!(
            matches!(result, Err(QueryError::Cancelled)),
            "checkpoint {stop}"
        );
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
        let result = plan.execute(
            &host,
            &[],
            ExecutionOptions {
                limits,
                ..options()
            },
        );
        assert!(matches!(result, Err(QueryError::Limit(_))));
    }
}

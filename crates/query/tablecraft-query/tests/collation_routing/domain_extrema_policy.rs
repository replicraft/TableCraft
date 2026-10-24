use super::support::*;
use std::cell::Cell;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_expressions::EvalError;
use tablecraft_query::QueryLimits;

fn fixture() -> Fixture {
    let mut source = Fixture::new();
    source.rows[0][2] = Value::Text("m".into());
    source
}

#[test]
fn extrema_wait_for_a_late_runtime_order_before_discarding_candidates() {
    let parts = ["\"'a'\"", "\"'z'\"", "\"'m'\"", "'B'"];
    for order in [[0, 1, 2, 3], [3, 0, 1, 2], [1, 0, 3, 2]] {
        let choices = order.map(|index| parts[index]).join(",");
        let source = fixture();
        let resolver = Resolver::new();
        let expression = format!("DLookup(Choose(Id,{choices}),'T','Id=1')");
        let sql = format!("SELECT Min({expression}),Max({expression}) FROM T");
        let parsed = parse_query(&sql, ParseLimits::default()).unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        assert_eq!(source.scans.get(), 0);
        assert!(resolver.hits.borrow().is_empty());

        let result = plan
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    collation: Some(&DEFAULT),
                    collation_resolver: Some(&resolver),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(
            result.rows(),
            [vec![Value::Text("z".into()), Value::Text("a".into())]]
        );
        // One outer scan and four domain calls per independent aggregate.
        assert_eq!(source.scans.get(), 9);
    }
}

#[test]
fn final_field_order_does_not_require_an_unused_default_comparer() {
    let source = fixture();
    let resolver = Resolver::new();
    let sql = concat!(
        "SELECT Min(DLookup(Choose(Id,\"'a'\",\"'z'\",\"'m'\",'B'),'T','Id=1')) ",
        "FROM T"
    );
    let parsed = parse_query(sql, ParseLimits::default()).unwrap();
    let result = prepare(parsed.query(), &source)
        .unwrap()
        .execute(
            &source,
            &[],
            ExecutionOptions {
                collation_resolver: Some(&resolver),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(result.rows(), texts(&["z"]));
}

#[test]
fn null_contributors_retain_policy_and_explicit_global_overrides_remain_stable() {
    let mut source = fixture();
    source.rows[0][2] = Value::Null;
    let resolver = Resolver::new();
    let expression = r#"DLookup(Choose(Id,"'a'","'z'","'m'",'B'),'T','Id=1')"#;
    let sql = format!("SELECT Min({expression}),Max({expression}) FROM T");
    assert_eq!(
        run(&sql, &source, Some(&resolver)).unwrap(),
        [vec![Value::Text("z".into()), Value::Text("a".into())]]
    );
    assert_eq!(
        run(&sql, &source, None).unwrap(),
        [vec![Value::Text("a".into()), Value::Text("z".into())]]
    );
}

#[test]
fn deferred_extrema_keep_all_cancellation_checkpoints_quotas_and_late_failures() {
    let source = fixture();
    let resolver = Resolver::new();
    let expression = r#"DLookup(Choose(Id,"'a'","'z'","'m'",'B'),'T','Id=1')"#;
    let sql = format!("SELECT Min({expression}),Max({expression}) FROM T");
    let query = parse_query(&sql, ParseLimits::default()).unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    let calls = Cell::new(0);
    let count = || {
        calls.set(calls.get() + 1);
        false
    };
    let options = ExecutionOptions {
        collation: Some(&DEFAULT),
        collation_resolver: Some(&resolver),
        cancelled: Some(&count),
        ..Default::default()
    };
    assert!(plan.execute(&source, &[], options).is_ok());
    let total = calls.get();
    for stop in 1..=total {
        calls.set(0);
        *resolver.budget.borrow_mut() = ResourceBudget::new(0, 0, 100_000);
        resolver.hits.borrow_mut().clear();
        let cancelled = || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        };
        assert!(matches!(
            plan.execute(
                &source,
                &[],
                ExecutionOptions {
                    cancelled: Some(&cancelled),
                    ..options
                },
            ),
            Err(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
        ));
    }
    assert!(matches!(
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                cancelled: None,
                limits: QueryLimits {
                    bytes: 1,
                    ..Default::default()
                },
                ..options
            },
        ),
        Err(QueryError::Limit(_))
    ));

    let bad = concat!(
        "SELECT Min(DLookup(Choose(Id,\"'a'\",\"'z'\",\"'m'\",'1/0'),'T','Id=1')) ",
        "FROM T"
    );
    assert!(matches!(
        run(bad, &source, Some(&resolver)),
        Err(QueryError::Evaluation(EvalError::DivisionByZero))
    ));
}

#[test]
fn deferred_singleton_extrema_keep_lossless_strings_without_unused_comparison() {
    let mut source = fixture();
    source.rows.truncate(1);
    let resolver = Resolver::new();
    let query = parse_query("SELECT LenB(Min(ChrW(55296))) FROM T", Default::default()).unwrap();
    let result = prepare(query.query(), &source)
        .unwrap()
        .execute(
            &source,
            &[],
            ExecutionOptions {
                collation_resolver: Some(&resolver),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(result.rows(), [vec![Value::Int32(2)]]);
    assert!(resolver.hits.borrow().is_empty());
}

#[test]
fn numeric_extrema_do_not_retain_a_string_candidate_buffer() {
    let source = fixture();
    let resolver = Resolver::new();
    let query = parse_query("SELECT Min(Id),Max(Id) FROM T", Default::default()).unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    let mut lower = 0;
    let mut upper = 65536;
    while lower < upper {
        let bytes = lower + (upper - lower) / 2;
        let result = plan.execute(
            &source,
            &[],
            ExecutionOptions {
                limits: QueryLimits {
                    bytes,
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        if result.is_ok() {
            upper = bytes;
        } else {
            assert!(matches!(result, Err(QueryError::Limit(_))));
            lower = bytes + 1;
        }
    }
    let result = plan
        .execute(
            &source,
            &[],
            ExecutionOptions {
                collation_resolver: Some(&resolver),
                limits: QueryLimits {
                    bytes: upper,
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(result.rows(), [vec![Value::Int32(1), Value::Int32(4)]]);
}

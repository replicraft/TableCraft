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

#[test]
fn completed_concatenation_extrema_use_the_final_runtime_policy() {
    let source = fixture();
    let resolver = Resolver::new();
    let expression = concat!(
        "DLookup(Choose(Id,\"'a'\",\"'z'\",\"'m'\",'B'),'T','Id=1')",
        " & IIf(True,False,A)",
    );
    let sql = format!("SELECT Min({expression}),Max({expression}) FROM T");
    let result = run(&sql, &source, Some(&resolver)).unwrap();
    let actual = result[0]
        .iter()
        .map(|value| match value {
            Value::Text(text) => text.as_str(),
            _ => panic!("expected synthetic Text result"),
        })
        .collect::<Vec<_>>();
    assert_eq!(actual, ["zFalse", "aFalse"]);
}

#[test]
fn completed_extrema_are_independent_of_encounter_order_and_derived_boundaries() {
    let parts = ["\"'a'\"", "\"'z'\"", "\"'m'\"", "'B'"];
    for first in 0..parts.len() {
        for second in 0..parts.len() {
            for third in 0..parts.len() {
                for fourth in 0..parts.len() {
                    let order = [first, second, third, fourth];
                    if order
                        .iter()
                        .enumerate()
                        .any(|(index, value)| order[..index].contains(value))
                    {
                        continue;
                    }
                    let choices = order.map(|index| parts[index]).join(",");
                    let expression =
                        format!("DLookup(Choose(Id,{choices}),'T','Id=1') & IIf(True,False,A)");
                    for declared in [false, true] {
                        let mut source = fixture();
                        source.named = parse_query(
                            &format!("SELECT {expression} AS S FROM T"),
                            Default::default(),
                        )
                        .unwrap()
                        .query()
                        .clone();
                        for sql in [
                            format!("SELECT Min({expression}),Max({expression}) FROM T"),
                            format!(
                                "SELECT Min(S),Max(S) FROM (SELECT {expression} AS S FROM T) Q"
                            ),
                            "SELECT Min(S),Max(S) FROM Named".into(),
                        ] {
                            let resolver = Resolver::new();
                            let syntax = parse_query(&sql, Default::default()).unwrap();
                            let result = prepare(syntax.query(), &source)
                                .unwrap()
                                .execute(
                                    &source,
                                    &[],
                                    ExecutionOptions {
                                        collation: Some(&DEFAULT),
                                        collation_resolver: Some(&resolver),
                                        declared_integral: declared,
                                        ..Default::default()
                                    },
                                )
                                .unwrap();
                            assert_eq!(
                                result.rows(),
                                [vec![
                                    Value::Text("zFalse".into()),
                                    Value::Text("aFalse".into())
                                ]],
                                "{sql}; order {order:?}; declared {declared}",
                            );
                            assert!(!resolver.hits.borrow().contains(&1));
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn completed_extrema_do_not_require_an_unused_default_or_repeat_domain_calls() {
    let source = fixture();
    let resolver = Resolver::new();
    let expression = concat!(
        "DLookup(Choose(Id,\"'a'\",\"'z'\",\"'m'\",'B'),'T','Id=1')",
        " & IIf(True,False,A)",
    );
    let syntax = parse_query(
        &format!("SELECT Min({expression}),Max({expression}) FROM T"),
        Default::default(),
    )
    .unwrap();
    let result = prepare(syntax.query(), &source)
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
    assert_eq!(
        result.rows(),
        [vec![
            Value::Text("zFalse".into()),
            Value::Text("aFalse".into())
        ]]
    );
    assert_eq!(source.scans.get(), 9);
    assert!(resolver.hits.borrow().iter().all(|policy| *policy == 2));
}

#[test]
fn complete_policy_selection_and_field_stability_proof_keep_cancellation_and_errors() {
    for sql in [
        concat!(
            "SELECT Min(DLookup(Choose(Id,\"'a'\",\"'z'\",\"'m'\",'B'),'T','Id=1') ",
            "& IIf(True,False,A)) FROM T",
        ),
        "SELECT Min(A),Max(CVar(A)) FROM T",
    ] {
        let source = fixture();
        let parsed = parse_query(sql, Default::default()).unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        let observed = Cell::new(0);
        let observe = || {
            observed.set(observed.get() + 1);
            false
        };
        let resolver = Resolver::new();
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                collation_resolver: Some(&resolver),
                cancelled: Some(&observe),
                ..Default::default()
            },
        )
        .unwrap();

        for stop in 1..=observed.get() {
            let calls = Cell::new(0);
            let cancel = || {
                calls.set(calls.get() + 1);
                calls.get() == stop
            };
            let resolver = Resolver::new();
            assert!(
                matches!(
                    plan.execute(
                        &source,
                        &[],
                        ExecutionOptions {
                            collation_resolver: Some(&resolver),
                            cancelled: Some(&cancel),
                            ..Default::default()
                        },
                    ),
                    Err(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
                ),
                "{sql}; checkpoint {stop}"
            );
        }
    }

    let source = fixture();
    let resolver = Resolver::new();
    let sql = concat!(
        "SELECT Min(DLookup(Choose(Id,\"'a'\",\"'z'\",\"'m'\",'1/0'),'T','Id=1') ",
        "& IIf(True,False,A)) FROM T",
    );
    assert_eq!(
        run(sql, &source, Some(&resolver)).err(),
        Some(QueryError::Evaluation(EvalError::DivisionByZero))
    );
}

#[test]
fn deferred_cohorts_keep_late_completion_and_null_policy_contributors() {
    for (last, null) in [("\"B & IIf(True,False,A)\"", false), ("'B'", true)] {
        let mut source = fixture();
        if null {
            source.rows[0][2] = Value::Null;
        }
        let resolver = Resolver::new();
        let (first, second, third, expected) = if null {
            ("\"'a'&''\"", "\"'z'&''\"", "\"'m'&''\"", ["z", "a"])
        } else {
            ("\"'a'\"", "\"'z'\"", "\"'m'\"", ["z", "a"])
        };
        let expression = format!("DLookup(Choose(Id,{first},{second},{third},{last}),'T','Id=1')");
        let sql = format!("SELECT Min({expression}),Max({expression}) FROM T");
        let parsed = parse_query(&sql, Default::default()).unwrap();
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
        let actual = result.rows()[0]
            .iter()
            .map(|value| match value {
                Value::Text(text) => text.as_str(),
                _ => panic!("expected synthetic Text result"),
            })
            .collect::<Vec<_>>();
        assert_eq!(actual, expected, "{sql}");
    }
}

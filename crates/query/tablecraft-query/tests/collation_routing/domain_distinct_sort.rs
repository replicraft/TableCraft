use super::support::*;
use std::cell::Cell;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_expressions::EvalError;
use tablecraft_query::QueryLimits;

#[test]
fn distinct_sort_keys_retain_policies_from_discarded_duplicate_rows() {
    let source = Fixture::new();
    let resolver = Resolver::new();
    let value = r#"DLookup(Choose(Id,"'a'",'B',"'b'","'b'"),'T','Id=1')"#;
    let sql = format!("SELECT DISTINCT {value} AS X FROM T ORDER BY X");

    assert_eq!(
        run(&sql, &source, Some(&resolver)).unwrap(),
        texts(&["b", "a"])
    );
    assert_eq!(run(&sql, &source, None).unwrap(), texts(&["a", "b"]));

    for (modifier, expected) in [
        ("DISTINCT TOP 1", texts(&["b"])),
        ("DISTINCT TOP 50 PERCENT", texts(&["b"])),
    ] {
        let sql = format!("SELECT {modifier} {value} AS X FROM T ORDER BY X");
        assert_eq!(run(&sql, &source, Some(&resolver)).unwrap(), expected);
    }

    let sql = concat!(
        "SELECT DISTINCT TOP 1 ",
        "DLookup(Choose(Id,\"'a'\",'A',\"'A'\",\"'z'\"),'T','Id=1') AS X,",
        "IIf(Id<3,1,2) AS Y FROM T ORDER BY X"
    );
    assert_eq!(
        run(sql, &source, Some(&resolver)).unwrap(),
        [
            vec![Value::Text("a".into()), Value::Int64(1)],
            vec![Value::Text("A".into()), Value::Int64(2)],
        ]
    );

    // A fresh text function keeps its documented Default result policy.
    let sql = format!("SELECT DISTINCT CStr({value}) AS X FROM T ORDER BY X");
    assert_eq!(
        run(&sql, &source, Some(&resolver)).unwrap(),
        texts(&["a", "b"])
    );
}

#[test]
fn distinct_sort_policy_retention_remains_metadata_only_and_eager() {
    let source = Fixture::new();
    let resolver = Resolver::new();
    let value = r#"DLookup(Choose(Id,"'a'",'B',"'b'","'b'"),'T','Id=1')"#;
    let sql = format!("SELECT DISTINCT {value} AS X FROM T ORDER BY X");
    let parsed = parse_query(&sql, ParseLimits::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    assert_eq!(source.scans.get(), 0);
    assert!(resolver.hits.borrow().is_empty());

    let observed = Cell::new(0);
    let count = || {
        observed.set(observed.get() + 1);
        false
    };
    let options = ExecutionOptions {
        collation: Some(&DEFAULT),
        collation_resolver: Some(&resolver),
        cancelled: Some(&count),
        ..Default::default()
    };
    assert_eq!(
        plan.execute(&source, &[], options).unwrap().rows(),
        texts(&["b", "a"])
    );

    for stop in 1..=observed.get() {
        let calls = Cell::new(0);
        let cancelled = || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        };
        *resolver.budget.borrow_mut() = ResourceBudget::new(0, 0, 10000);
        assert!(
            matches!(
                plan.execute(
                    &source,
                    &[],
                    ExecutionOptions {
                        collation: Some(&DEFAULT),
                        collation_resolver: Some(&resolver),
                        cancelled: Some(&cancelled),
                        ..Default::default()
                    },
                ),
                Err(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
            ),
            "checkpoint {stop}"
        );
    }

    assert!(matches!(
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                limits: QueryLimits {
                    bytes: 1,
                    ..Default::default()
                },
                collation: Some(&DEFAULT),
                collation_resolver: Some(&resolver),
                ..Default::default()
            },
        ),
        Err(QueryError::Limit(_))
    ));

    let sql = r#"SELECT DISTINCT DLookup(Choose(Id,"'a'",'B',"1/0","'b'"),'T','Id=1') AS X FROM T ORDER BY X"#;
    assert!(matches!(
        run(sql, &source, Some(&resolver)),
        Err(QueryError::Evaluation(EvalError::DivisionByZero))
    ));
}

#[test]
fn distinct_sort_policy_retention_does_not_depend_on_duplicate_position() {
    let source = Fixture::new();
    for choices in [
        r#""'a'",'B',"'b'","'b'""#,
        r#""'a'","'b'",'B',"'b'""#,
        r#""'a'","'b'","'b'",'B'"#,
        r#"'B',"'a'","'b'","'b'""#,
    ] {
        for (direction, expected) in [("ASC", texts(&["b", "a"])), ("DESC", texts(&["a", "b"]))] {
            let resolver = Resolver::new();
            let sql = format!(
                "SELECT DISTINCT DLookup(Choose(Id,{choices}),'T','Id=1') AS X FROM T ORDER BY X {direction}"
            );
            assert_eq!(run(&sql, &source, Some(&resolver)).unwrap(), expected);
        }
    }
}

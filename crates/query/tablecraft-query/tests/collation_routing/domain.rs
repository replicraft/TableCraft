use super::support::*;
use std::cell::Cell;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_core::values::ExpressionValue;
use tablecraft_expressions::EvalError;
use tablecraft_query::{QueryLimits, execute_with_schema_discovery};

#[test]
fn domain_criteria_extrema_and_returned_values_use_captured_field_policies() {
    let source = Fixture::new();
    let resolver = Resolver::new();
    let rows = run(
        "SELECT DCount('*','T',\"A='A'\"),DMin('B','T'),DMax('B','T')",
        &source,
        Some(&resolver),
    )
    .unwrap();
    assert_eq!(
        rows,
        [vec![
            Value::Int64(2),
            Value::Text("a".into()),
            Value::Text("A".into())
        ]]
    );
    assert!(resolver.hits.borrow().contains(&1));
    assert!(resolver.hits.borrow().contains(&2));
    let rows = run("SELECT DMin('B','T')='A'", &source, Some(&resolver)).unwrap();
    assert_eq!(rows, [vec![Value::Boolean(false)]]);
    assert!(run("SELECT DMin('U','T')", &source, Some(&resolver)).is_err());
}

#[test]
fn runtime_domain_policies_survive_selectors_derived_rows_predicates_and_keys() {
    let source = Fixture::new();
    let resolver = Resolver::new();
    for sql in [
        "SELECT DLookup('A','T')='A'",
        "SELECT DFirst('A','T')='A'",
        "SELECT DLast('A','T')='B'",
        "SELECT DMin('A','T')='A'",
        "SELECT DMax('A','T')='B'",
        "SELECT Min(DLookup('A','T'))='A' FROM T",
        "SELECT First(DLookup('A','T'))='A' FROM T",
        "SELECT DMin('B','T')<'A'",
        "SELECT X='A' FROM (SELECT DLookup('A','T') AS X) AS Q",
        "SELECT (DLookup('A','T')&'')='A'",
        "SELECT IIf(True,DLookup('A','T'),'x')='A'",
        "SELECT Choose(1,DLookup('A','T'))='A'",
        "SELECT Switch(True,DLookup('A','T'))='A'",
        "SELECT Nz(DLookup('A','T'),'x')='A'",
        "SELECT DLookup('A','T') IN ('A')",
        "SELECT DLookup('A','T')=ANY (SELECT 'A')",
        "SELECT 'A' IN (SELECT DLookup('A','T'))",
    ] {
        assert_eq!(
            run(sql, &source, Some(&resolver)).unwrap(),
            [vec![Value::Boolean(true)]],
            "{sql}"
        );
    }
    for (sql, expected) in [
        (
            "SELECT Id FROM (SELECT Id,DLookup(Choose(Id,'A','B','A','B'),'T') AS X FROM T) AS Q WHERE X='A' ORDER BY Id",
            ids(&[1, 3]),
        ),
        (
            "SELECT Id FROM T ORDER BY DLookup('B','T',Choose(Id,'Id=1','Id=2','Id=3','Id=4')),Id",
            ids(&[1, 3, 4, 2]),
        ),
        (
            "SELECT TOP 1 Id FROM T ORDER BY DLookup('B','T',Choose(Id,'Id=1','Id=2','Id=3','Id=4'))",
            ids(&[1, 3]),
        ),
        (
            "SELECT L.Id FROM (SELECT Id,DLookup('A','T') AS X FROM T) AS L INNER JOIN (SELECT 'A' AS Y) AS R ON L.X=R.Y ORDER BY L.Id",
            ids(&[1, 2, 3, 4]),
        ),
        (
            "SELECT Count(*) FROM T GROUP BY DLookup('A','T')",
            vec![vec![Value::Int64(4)]],
        ),
        ("SELECT DISTINCT DLookup('A','T') FROM T", texts(&["a"])),
        (
            "SELECT DLookup('A','T') AS X UNION SELECT 'A'",
            texts(&["a"]),
        ),
        (
            "TRANSFORM Count(*) SELECT 1 AS G FROM T GROUP BY 1 PIVOT DLookup('A','T',Choose(Id,'Id=1','Id=2','Id=3','Id=4'))",
            vec![vec![Value::Int64(1), Value::Int64(2), Value::Int64(2)]],
        ),
    ] {
        assert_eq!(
            run(sql, &source, Some(&resolver)).unwrap(),
            expected,
            "{sql}"
        );
    }
}

#[test]
fn runtime_domain_policy_conflicts_and_unknowns_never_fall_back_or_activate_on_projection() {
    let source = Fixture::new();
    let resolver = Resolver::new();
    for sql in [
        "SELECT DLookup('A','T')=DLookup('B','T')",
        "SELECT IIf(True,DLookup('A','T'),DLookup('B','T'))='A'",
        "SELECT DLookup('A','T') AS X UNION SELECT DLookup('B','T')",
        "SELECT Count(*) FROM T GROUP BY DLookup(Choose(Id,'A','B','A','B'),'T')",
        "SELECT Min(DLookup(Choose(Id,'A','B','A','B'),'T')) FROM T",
        "SELECT First(DLookup(Choose(Id,'A','B','A','B'),'T'))='A' FROM T",
    ] {
        assert_eq!(
            run(sql, &source, Some(&resolver)).err(),
            Some(QueryError::CollationConflict),
            "{sql}"
        );
        assert!(run(sql, &source, None).is_ok(), "global override: {sql}");
    }
    assert_eq!(
        run(
            "SELECT Left(DLookup('A','T'),1)='A'",
            &source,
            Some(&resolver)
        )
        .unwrap(),
        [vec![Value::Boolean(false)]]
    );
    resolver.hits.borrow_mut().clear();
    assert!(run("SELECT DLookup('U','T')", &source, Some(&resolver)).is_ok());
    assert!(resolver.hits.borrow().is_empty());
    assert!(matches!(
        run("SELECT DLookup('U','T')='x'", &source, Some(&resolver)),
        Err(QueryError::Evaluation(EvalError::CollationRequired))
    ));
}

#[test]
fn runtime_domain_policy_survives_explicit_named_crosstab_materialization() {
    let mut source = Fixture::new();
    source.named = parse_query(
        "TRANSFORM First(DLookup('A','T')) SELECT 1 AS G FROM T GROUP BY 1 PIVOT Id",
        ParseLimits::default(),
    )
    .unwrap()
    .query()
    .clone();
    let query = parse_query("SELECT [1]='A' FROM Named", ParseLimits::default()).unwrap();
    let resolver = Resolver::new();
    let result = execute_with_schema_discovery(
        query.query(),
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
        [vec![ExpressionValue::Value(Value::Boolean(true))]]
    );
}

#[test]
fn runtime_domain_metadata_is_inert_parameters_rebind_and_cancellation_is_shared() {
    let source = Fixture::new();
    let resolver = Resolver::new();
    let syntax = parse_query(
        "PARAMETERS P TEXT; SELECT DLookup(P,'T')='A'",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    assert_eq!(source.scans.get(), 0);
    for (field, expected) in [("A", true), ("B", false), ("A", true)] {
        let result = plan
            .execute(
                &source,
                &[("P", Value::Text(field.into()))],
                ExecutionOptions {
                    collation: Some(&DEFAULT),
                    collation_resolver: Some(&resolver),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(result.rows(), [vec![Value::Boolean(expected)]]);
    }
    let parameters = [("P", Value::Text("A".into()))];
    let observed = Cell::new(0);
    let callback = || {
        observed.set(observed.get() + 1);
        false
    };
    plan.execute_expression(
        &source,
        &parameters,
        ExecutionOptions {
            collation: Some(&DEFAULT),
            collation_resolver: Some(&resolver),
            cancelled: Some(&callback),
            ..Default::default()
        },
    )
    .unwrap();
    for stop in 1..=observed.get() {
        let calls = Cell::new(0);
        let cancelled = || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        };
        *resolver.budget.borrow_mut() = ResourceBudget::new(0, 0, 10000);
        assert!(matches!(
            plan.execute_expression(
                &source,
                &parameters,
                ExecutionOptions {
                    collation: Some(&DEFAULT),
                    collation_resolver: Some(&resolver),
                    cancelled: Some(&cancelled),
                    ..Default::default()
                }
            ),
            Err(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
        ));
    }
    assert!(matches!(
        plan.execute_expression(
            &source,
            &parameters,
            ExecutionOptions {
                limits: QueryLimits {
                    bytes: 1,
                    ..Default::default()
                },
                ..Default::default()
            }
        ),
        Err(QueryError::Limit(_))
    ));
}

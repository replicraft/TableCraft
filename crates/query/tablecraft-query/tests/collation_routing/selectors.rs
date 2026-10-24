use super::support::*;
use std::cell::Cell;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_expressions::EvalError;
use tablecraft_query::source::TextPolicy;

#[test]
fn selector_outputs_retain_value_policies_without_resolving_metadata_or_index_policies() {
    for expression in [
        "Choose(1,A)",
        "Choose(Len(U)/Len(U),A,A)",
        "Switch(True,A)",
        "Switch(Len(U)>0,A,True,A)",
        "Choose(1,Switch(True,A),A)",
        "Switch(True,Choose(1,A),False,A)",
        "A & Left(ChrW(55296),0)",
        "Left(ChrW(55296),0) & (SELECT T.A)",
        "CVar((SELECT T.A)) & Left(ChrW(55296),0)",
    ] {
        let source = Fixture::new();
        let resolver = Resolver::new();
        let sql = format!("SELECT {expression} AS Label FROM T");
        let syntax = parse_query(&sql, ParseLimits::default()).unwrap();
        let plan = prepare(syntax.query(), &source).unwrap();
        assert_eq!(
            plan.output_text_policies(),
            Some(&[TextPolicy::Column(TextCollationId::from_host(1))][..])
        );
        assert_eq!(source.scans.get(), 0);
        assert!(resolver.hits.borrow().is_empty());
        assert_eq!(
            run(
                &format!("SELECT Id FROM T WHERE {expression}='A' ORDER BY Id"),
                &source,
                Some(&resolver)
            )
            .unwrap(),
            ids(&[1, 2])
        );
        assert!(resolver.hits.borrow().iter().all(|key| *key == 1));
    }
}

#[test]
fn selector_lineage_survives_relational_keys_aliases_and_subqueries() {
    let source = Fixture::new();
    let resolver = Resolver::new();
    for (sql, expected) in [
        (
            "SELECT Id FROM (SELECT Id,Choose(1,A) AS X FROM T) AS Q WHERE X='A' ORDER BY Id",
            ids(&[1, 2]),
        ),
        (
            "SELECT Id FROM T WHERE Switch(True,A) IN ('A') ORDER BY Id",
            ids(&[1, 2]),
        ),
        (
            "SELECT Id FROM T WHERE Choose(1,A) LIKE 'A' ORDER BY Id",
            ids(&[1, 2]),
        ),
        (
            "SELECT Id FROM T WHERE Switch(True,A) BETWEEN 'A' AND 'a' ORDER BY Id",
            ids(&[1, 2]),
        ),
        (
            "SELECT Id FROM T WHERE Choose(1,A)=ANY (SELECT 'A') ORDER BY Id",
            ids(&[1, 2]),
        ),
        (
            "SELECT Id FROM T WHERE (SELECT Switch(True,T.A))='A' ORDER BY Id",
            ids(&[1, 2]),
        ),
        (
            "SELECT Count(*) FROM T GROUP BY Choose(1,A) ORDER BY Min(Id)",
            vec![vec![Value::Int64(2)], vec![Value::Int64(2)]],
        ),
        (
            "SELECT DISTINCT Switch(True,A) FROM T ORDER BY 1",
            texts(&["a", "b"]),
        ),
        (
            "SELECT Choose(1,A) FROM T UNION SELECT 'A' ORDER BY 1",
            texts(&["a", "b"]),
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
fn conflicting_and_unknown_selector_results_do_not_silently_use_the_default() {
    let source = Fixture::new();
    let resolver = Resolver::new();
    for expression in ["Choose(1,A,B)", "Switch(True,A,False,B)"] {
        assert!(
            run(
                &format!("SELECT {expression} FROM T"),
                &source,
                Some(&resolver)
            )
            .is_ok()
        );
        for sql in [
            format!("SELECT {expression}='A' FROM T"),
            format!("SELECT DISTINCT {expression} FROM T"),
            format!("SELECT Count(*) FROM T GROUP BY {expression}"),
        ] {
            assert_eq!(
                run(&sql, &source, Some(&resolver)).err(),
                Some(QueryError::CollationConflict),
                "{sql}"
            );
        }
        assert!(run(&format!("SELECT {expression}='A' FROM T"), &source, None).is_ok());
    }
    assert!(matches!(
        run("SELECT Choose(1,U)='x' FROM T", &source, Some(&resolver)),
        Err(QueryError::Evaluation(EvalError::CollationRequired))
    ));
    assert!(run("SELECT Switch(True,U) FROM T", &source, Some(&resolver)).is_ok());
    assert!(matches!(
        run(
            "SELECT Switch(True,A,False,1/0)='A' FROM T",
            &source,
            Some(&resolver)
        ),
        Err(QueryError::Evaluation(EvalError::DivisionByZero))
    ));
}

#[test]
fn selector_lineage_work_shares_limits_and_every_observed_cancellation_checkpoint() {
    let source = Fixture::new();
    let resolver = Resolver::new();
    let syntax = parse_query(
        "SELECT Choose(1,Switch(True,A),A)='A' FROM T",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    let visits = Cell::new(0);
    let stop = Cell::new(usize::MAX);
    let cancelled = || {
        visits.set(visits.get() + 1);
        visits.get() == stop.get()
    };
    let options = || ExecutionOptions {
        collation: Some(&DEFAULT),
        collation_resolver: Some(&resolver),
        cancelled: Some(&cancelled),
        ..Default::default()
    };
    plan.execute(&source, &[], options()).unwrap();
    let count = visits.get();
    for point in 1..=count {
        visits.set(0);
        stop.set(point);
        *resolver.budget.borrow_mut() = ResourceBudget::new(0, 0, 10000);
        assert!(matches!(
            plan.execute(&source, &[], options()),
            Err(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
        ));
    }
    assert!(matches!(
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                collation: Some(&DEFAULT),
                collation_resolver: Some(&resolver),
                limits: tablecraft_query::QueryLimits {
                    steps: 1,
                    ..Default::default()
                },
                ..Default::default()
            }
        ),
        Err(QueryError::Limit(_))
    ));
}

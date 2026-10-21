use super::support::*;
use tablecraft_expressions::EvalError;
#[test]
fn distinct_field_policies_route_scalar_filter_like_membership_and_join_comparisons() {
    let source = Fixture::new();
    let resolver = Resolver::new();
    for (sql, expected) in [
        ("SELECT Id FROM T WHERE A='A' ORDER BY Id", ids(&[1, 2])),
        (
            "SELECT Id FROM T WHERE CVar(A)='A' ORDER BY Id",
            ids(&[1, 2]),
        ),
        ("SELECT Id FROM T WHERE CVar(B)='A' ORDER BY Id", ids(&[2])),
        (
            "SELECT t.Id FROM T t WHERE (SELECT t.A)='A' ORDER BY t.Id",
            ids(&[1, 2]),
        ),
        (
            "SELECT Id FROM T WHERE (A&'')='A' ORDER BY Id",
            ids(&[1, 2]),
        ),
        (
            "SELECT Id FROM T WHERE IIf(TRUE,A,'x')='A' ORDER BY Id",
            ids(&[1, 2]),
        ),
        ("SELECT Id FROM T WHERE Nz(A)='A' ORDER BY Id", ids(&[1, 2])),
        (
            "SELECT Id FROM T WHERE Left(A,1)='A' ORDER BY Id",
            ids(&[2]),
        ),
        ("SELECT Id FROM T WHERE B='A' ORDER BY Id", ids(&[2])),
        (
            "SELECT Id FROM T WHERE A LIKE 'a' ORDER BY Id",
            ids(&[1, 2]),
        ),
        (
            "SELECT Id FROM T WHERE A BETWEEN 'A' AND 'a' ORDER BY Id",
            ids(&[1, 2]),
        ),
        (
            "SELECT Id FROM T WHERE A IN ('A','z') ORDER BY Id",
            ids(&[1, 2]),
        ),
        (
            "SELECT Id FROM T WHERE A IN (SELECT 'A') ORDER BY Id",
            ids(&[1, 2]),
        ),
        (
            "SELECT Id FROM T WHERE A=ANY(SELECT A FROM T WHERE Id=2) ORDER BY Id",
            ids(&[1, 2]),
        ),
        (
            "SELECT Id FROM T WHERE A=ALL(SELECT 'A') ORDER BY Id",
            ids(&[1, 2]),
        ),
        (
            "SELECT n.Id FROM Named n WHERE n.A='A' ORDER BY n.Id",
            ids(&[1, 2]),
        ),
        (
            "SELECT q.Id FROM (SELECT Id,A FROM T) q WHERE q.A='A' ORDER BY q.Id",
            ids(&[1, 2]),
        ),
        (
            "SELECT t.Id FROM T t WHERE EXISTS(SELECT 1 WHERE t.A='A') ORDER BY t.Id",
            ids(&[1, 2]),
        ),
    ] {
        assert_eq!(
            run(sql, &source, Some(&resolver)).unwrap(),
            expected,
            "{sql}"
        );
    }
    let result = run(
        "SELECT l.Id,r.Id FROM T l INNER JOIN T r ON l.A=r.A WHERE l.Id=1 ORDER BY r.Id",
        &source,
        Some(&resolver),
    )
    .unwrap();
    assert_eq!(
        result,
        [
            vec![Value::Int32(1), Value::Int32(1)],
            vec![Value::Int32(1), Value::Int32(2)]
        ]
    );
    assert_eq!(
        run("SELECT Id FROM T WHERE A='A' ORDER BY Id", &source, None).unwrap(),
        ids(&[2])
    );
    assert!(resolver.hits.borrow().contains(&1));
    assert!(resolver.hits.borrow().contains(&2));
}
#[test]
fn conflicts_and_unknown_policies_fail_only_for_operations_requiring_text_comparison() {
    let source = Fixture::new();
    let resolver = Resolver::new();
    for sql in [
        "SELECT A=B FROM T",
        "SELECT CVar(A)=CVar(B) FROM T",
        "SELECT A IN ('A',B) FROM T",
        "SELECT A IN (SELECT B FROM T) FROM T",
        "SELECT DISTINCT IIf(TRUE,A,B) FROM T",
        "SELECT A FROM T UNION SELECT B FROM T",
    ] {
        assert_eq!(
            run(sql, &source, Some(&resolver)).err(),
            Some(QueryError::CollationConflict),
            "{sql}"
        );
    }
    assert!(matches!(
        run("SELECT U='x' FROM T", &source, Some(&resolver)),
        Err(QueryError::Evaluation(EvalError::CollationRequired))
    ));
    resolver.hits.borrow_mut().clear();
    assert_eq!(
        run("SELECT U=NULL FROM T", &source, Some(&resolver)).unwrap(),
        vec![vec![Value::Null]; 4]
    );
    assert!(run("SELECT U FROM T", &source, Some(&resolver)).is_ok());
    assert!(resolver.hits.borrow().is_empty());
    assert!(matches!(
        run("SELECT A=Id FROM T", &source, Some(&resolver)),
        Err(QueryError::Evaluation(EvalError::TypeMismatch))
    ));
    assert!(resolver.hits.borrow().is_empty());
    assert!(run("SELECT A=B FROM T", &source, None).is_ok());
}
#[test]
fn builtins_keep_host_database_policy_and_eager_errors_are_preserved() {
    let source = Fixture::new();
    let resolver = Resolver::new();
    assert_eq!(
        run(
            "SELECT A='A',StrComp(A,'A',2) FROM T WHERE Id=1",
            &source,
            Some(&resolver)
        )
        .unwrap(),
        [vec![Value::Boolean(true), Value::Int16(1)]]
    );
    for sql in [
        "SELECT A IN ('A',1/0) FROM T",
        "SELECT IIf(TRUE,A,1/0)='A' FROM T",
    ] {
        assert!(matches!(
            run(sql, &source, Some(&resolver)),
            Err(QueryError::Evaluation(EvalError::DivisionByZero))
        ));
    }
    let parsed = parse_query("SELECT U LIKE 'x' FROM T", ParseLimits::default()).unwrap();
    resolver.hits.borrow_mut().clear();
    let result = prepare(parsed.query(), &source).unwrap().execute(
        &source,
        &[],
        ExecutionOptions {
            collation: Some(&DEFAULT),
            collation_resolver: Some(&resolver),
            ..Default::default()
        },
    );
    assert!(matches!(
        result,
        Err(QueryError::Evaluation(EvalError::LikeModeRequired))
    ));
    assert!(resolver.hits.borrow().is_empty());
}

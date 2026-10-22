use super::support::*;
use tablecraft_expressions::{EvalError, functions::conversions::text::NumberFormat};
#[test]
fn grouping_distinct_min_max_sorting_and_top_ties_use_the_key_policy() {
    let source = Fixture::new();
    let resolver = Resolver::new();
    for (sql, expected) in [
        ("SELECT DISTINCT A FROM T ORDER BY A", texts(&["a", "b"])),
        (
            "SELECT DISTINCT T.A FROM T ORDER BY T.A",
            texts(&["a", "b"]),
        ),
        (
            "SELECT DISTINCT CVar(A) FROM T ORDER BY CVar(A)",
            texts(&["a", "b"]),
        ),
        (
            "SELECT DISTINCT CVar(B) FROM T ORDER BY CVar(B)",
            texts(&["a", "B", "A"]),
        ),
        (
            "SELECT DISTINCT B FROM T ORDER BY B",
            texts(&["a", "B", "A"]),
        ),
        (
            "SELECT A FROM T GROUP BY A HAVING Max(A)='A' ORDER BY A",
            texts(&["a"]),
        ),
        ("SELECT Id FROM T ORDER BY B,Id", ids(&[1, 3, 4, 2])),
        ("SELECT TOP 1 Id FROM T ORDER BY A", ids(&[1, 2])),
        ("SELECT TOP 1 Id FROM T ORDER BY B", ids(&[1, 3])),
        (
            "SELECT A FROM T WHERE Id=1 UNION SELECT A FROM T WHERE Id=2",
            texts(&["a"]),
        ),
        (
            "SELECT B FROM T WHERE Id=1 UNION SELECT B FROM T WHERE Id=2 ORDER BY B",
            texts(&["a", "A"]),
        ),
        (
            "SELECT A FROM T WHERE Id=1 UNION SELECT A FROM T WHERE Id=2 UNION ALL SELECT B FROM T WHERE Id=2",
            texts(&["a", "A"]),
        ),
    ] {
        assert_eq!(
            run(sql, &source, Some(&resolver)).unwrap(),
            expected,
            "{sql}"
        );
    }
    assert_eq!(
        run(
            "SELECT Min(A),Max(A),Min(B),Max(B) FROM T",
            &source,
            Some(&resolver)
        )
        .unwrap(),
        [vec![
            Value::Text("a".into()),
            Value::Text("b".into()),
            Value::Text("a".into()),
            Value::Text("A".into())
        ]]
    );
}
#[test]
fn distinctrow_retains_each_complete_record_fields_policy() {
    let mut source = Fixture::new();
    let resolver = Resolver::new();
    source.rows.truncate(2);
    source.rows[1] = source.rows[0].clone();
    source.rows[1][1] = Value::Text("A".into());
    for row in &mut source.rows {
        row[4] = Value::Null;
    }
    assert_eq!(
        run(
            "SELECT DISTINCTROW l.A FROM T l,T r",
            &source,
            Some(&resolver)
        )
        .unwrap(),
        texts(&["a"])
    );
    assert_eq!(
        run(
            "SELECT DISTINCTROW l.A FROM (SELECT * FROM T) l,T r",
            &source,
            Some(&resolver)
        )
        .unwrap(),
        texts(&["a"])
    );
    assert_eq!(
        run("SELECT DISTINCTROW l.A FROM T l,T r", &source, None).unwrap(),
        texts(&["a", "A"])
    );
}

#[test]
fn numeric_to_text_union_conversion_keeps_field_identity_and_set_stage_routing() {
    let source = Fixture::new();
    let resolver = Resolver::new();
    let query = parse_query(
        "SELECT Id AS N FROM T WHERE Id=1 UNION SELECT A FROM T WHERE Id=1",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    assert!(resolver.hits.borrow().is_empty());
    let options = ExecutionOptions {
        number_format: Some(NumberFormat::EnUs),
        collation: Some(&DEFAULT),
        collation_resolver: Some(&resolver),
        ..Default::default()
    };
    assert_eq!(
        plan.execute(&source, &[], options).unwrap().rows(),
        texts(&["1", "a"])
    );
    assert!(!resolver.hits.borrow().is_empty());
    assert!(resolver.hits.borrow().iter().all(|&key| key == 1));
    for (sql, expected) in [
        (
            "SELECT Id AS N FROM T WHERE Id=1 UNION SELECT U FROM T WHERE Id=1",
            QueryError::Evaluation(EvalError::CollationRequired),
        ),
        (
            "SELECT Id AS N FROM T WHERE Id=1 UNION SELECT A FROM T WHERE Id=1 UNION SELECT B FROM T WHERE Id=1",
            QueryError::CollationConflict,
        ),
    ] {
        let query = parse_query(sql, Default::default()).unwrap();
        assert_eq!(
            prepare(query.query(), &source)
                .unwrap()
                .execute(&source, &[], options)
                .err(),
            Some(expected)
        );
    }
    // A later ALL arm retains the established prefix policy for earlier set
    // stages; final ordering still uses the complete output's provenance.
    let sql = "SELECT 1 AS N UNION SELECT 2 UNION ALL SELECT U FROM T WHERE False";
    let query = parse_query(sql, Default::default()).unwrap();
    assert_eq!(
        prepare(query.query(), &source)
            .unwrap()
            .execute(&source, &[], options)
            .unwrap()
            .rows(),
        texts(&["1", "2"])
    );
    let query = parse_query(&format!("{sql} ORDER BY N"), Default::default()).unwrap();
    assert_eq!(
        prepare(query.query(), &source)
            .unwrap()
            .execute(&source, &[], options)
            .err(),
        Some(QueryError::Evaluation(EvalError::CollationRequired))
    );
}

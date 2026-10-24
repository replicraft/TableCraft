use super::*;

#[test]
fn repeated_units_survive_derived_grouped_correlated_and_domain_cells() {
    assert_eq!(
        run("SELECT LenB(S) FROM (SELECT String(N,ChrW(55296)) AS S FROM Words) AS Q").unwrap(),
        [
            vec![Value::Int32(4)],
            vec![Value::Int32(0)],
            vec![Value::Int32(6)]
        ],
    );
    assert_eq!(
        run("SELECT LenB(String(N,ChrW(55296))) AS Width,Count(*) FROM Words GROUP BY LenB(String(N,ChrW(55296))) ORDER BY Width").unwrap(),
        [vec![Value::Int32(0), Value::Int64(1)], vec![Value::Int32(4), Value::Int64(1)], vec![Value::Int32(6), Value::Int64(1)]],
    );
    assert_eq!(
        run("SELECT (SELECT LenB(String(W.N,ChrW(55296)))) FROM Words AS W").unwrap(),
        [
            vec![Value::Int32(4)],
            vec![Value::Int32(0)],
            vec![Value::Int32(6)]
        ],
    );
    assert_eq!(
        run("SELECT LenB(DLookup('String(N,ChrW(55296))','Words'))").unwrap(),
        [vec![Value::Int32(4)]],
    );
}

#[test]
fn prepared_string_parameters_rebind_counts_character_sources_and_null_gates() {
    let source = source();
    let parsed = parse_query(
        "PARAMETERS N LONG,P TEXT; SELECT LenB(String(N,P))",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    for (n, p, expected) in [
        (3, Value::Text("😀".into()), Value::Int32(6)),
        (2, Value::Text("ab".into()), Value::Int32(4)),
        (-1, Value::Null, Value::Null),
    ] {
        let result = plan
            .execute(
                &source,
                &[("N", Value::Int32(n)), ("P", p)],
                ExecutionOptions::default(),
            )
            .unwrap();
        assert_eq!(result.rows(), [vec![expected]]);
    }
    assert_eq!(
        run("SELECT String(0,'😀'),String(1,ChrW(55357))&ChrW(56832),String(ChrB(65),NULL)")
            .unwrap(),
        [vec![
            Value::Text(String::new()),
            Value::Text("😀".into()),
            Value::Null
        ]],
    );
}

#[test]
fn invalid_final_output_odd_character_ranges_and_later_failures_remain_buffered_errors() {
    for sql in ["SELECT String(2,'😀')", "SELECT LenB(String(0,ChrB(65)))"] {
        assert!(matches!(
            run(sql),
            Err(QueryError::Evaluation(EvalError::UnrepresentableText))
        ));
    }
    assert_eq!(
        run("SELECT LenB(String(2,ChrW(55296))),1/0"),
        Err(QueryError::Evaluation(EvalError::DivisionByZero)),
    );
}

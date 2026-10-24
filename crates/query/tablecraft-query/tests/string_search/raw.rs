use super::*;
use tablecraft_expressions::EvalError;

#[test]
fn retained_binary_searches_compose_with_derived_groups_and_correlations() {
    assert_eq!(
        run("SELECT InStr(1,S,ChrW(55296),0),InStrRev(S,ChrW(55296)) FROM (SELECT ChrW(55296)&'a'&ChrW(55296) AS S FROM Words) AS Q").unwrap(),
        vec![vec![Value::Int32(1), Value::Int32(3)]; 4],
    );
    assert_eq!(
        run("SELECT InStr(1,S,ChrW(55296),0) AS P,Count(*) FROM (SELECT ChrW(55296)&Label AS S FROM Words) AS Q GROUP BY InStr(1,S,ChrW(55296),0) HAVING Count(*)>1").unwrap(),
        [vec![Value::Int32(1), Value::Int64(4)]],
    );
    assert_eq!(
        run("SELECT (SELECT InStrRev(ChrW(55296)&W.Label,ChrW(55296))) FROM Words AS W").unwrap(),
        [
            vec![Value::Int32(1)],
            vec![Value::Int32(1)],
            vec![Value::Int32(1)],
            vec![Value::Int32(1)]
        ],
    );
}

#[test]
fn prepared_raw_search_parameters_rebind_without_unicode_publication() {
    let source = source();
    let query = parse_query(
        "PARAMETERS P LONG; SELECT InStr(1,ChrW(P)&ChrW(55296),ChrW(55296),0),InStrRev(ChrW(P)&ChrW(55296),ChrW(55296))",
        ParseLimits::default(),
    ).unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    for (value, first) in [(65, 2), (55296, 1), (57343, 2)] {
        let result = plan
            .execute(
                &source,
                &[("P", Value::Int32(value))],
                ExecutionOptions::default(),
            )
            .unwrap();
        assert_eq!(result.rows(), [vec![Value::Int32(first), Value::Int32(2)]]);
    }
}

#[test]
fn binary_search_failures_are_eager_and_do_not_publish_an_earlier_result() {
    for sql in [
        "SELECT InStrRev(ChrW(55296),ChrW(55296)),1/0",
        "SELECT InStr(1,ChrB(65),NULL,1/0)",
    ] {
        assert!(matches!(
            run(sql),
            Err(QueryError::Evaluation(EvalError::DivisionByZero))
        ));
    }
    assert!(matches!(
        run("SELECT InStrRev(ChrB(65),'')"),
        Err(QueryError::Evaluation(EvalError::UnrepresentableText))
    ));
}

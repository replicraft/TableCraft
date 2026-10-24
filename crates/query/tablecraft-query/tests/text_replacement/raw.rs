use super::*;
use tablecraft_expressions::EvalError;

#[test]
fn replacement_cells_survive_derived_groups_correlations_and_domains() {
    assert_eq!(
        run("SELECT LenB(S) FROM (SELECT Replace(ChrW(55296)&Label,'an',ChrW(57343)) AS S FROM Words) AS Q").unwrap(),
        [vec![Value::Int32(10)], vec![Value::Int32(12)], vec![Value::Int32(14)]],
    );
    assert_eq!(
        run("SELECT LenB(Replace(ChrW(55296)&Label,'an',ChrW(57343))) AS Width,Count(*) FROM Words GROUP BY LenB(Replace(ChrW(55296)&Label,'an',ChrW(57343))) ORDER BY Width").unwrap(),
        [vec![Value::Int32(10), Value::Int64(1)], vec![Value::Int32(12), Value::Int64(1)], vec![Value::Int32(14), Value::Int64(1)]],
    );
    assert_eq!(
        run("SELECT (SELECT LenB(Replace(ChrW(55296)&W.Label,'an',ChrW(57343)))) FROM Words AS W")
            .unwrap(),
        [
            vec![Value::Int32(10)],
            vec![Value::Int32(12)],
            vec![Value::Int32(14)]
        ],
    );
    assert_eq!(
        run("SELECT LenB(DLookup('Replace(ChrW(55296)&Label,ChrW(97),ChrW(57343))','Words'))")
            .unwrap(),
        [vec![Value::Int32(14)]],
    );
}

#[test]
fn prepared_replacement_rebinds_parameters_and_unit_suffixes_remain_observable() {
    let source = source();
    let query = parse_query(
        "PARAMETERS P LONG; SELECT AscW(Replace(ChrW(P),'z','',1,0))",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    for (input, expected) in [(65, 65), (55296, -10240), (57343, -8193)] {
        let result = plan
            .execute(
                &source,
                &[("P", Value::Int32(input))],
                ExecutionOptions::default(),
            )
            .unwrap();
        assert_eq!(result.rows(), [vec![Value::Int16(expected)]]);
    }
    assert_eq!(
        run("SELECT AscW(Replace('😀','z','',2,0)),LenB(Replace('😀','z','',2,0))").unwrap(),
        [vec![Value::Int16(-8704), Value::Int32(2)]],
    );
    assert_eq!(
        run("SELECT Replace(ChrW(55296)&'a','a',ChrW(56320))").unwrap(),
        [vec![Value::Text("\u{10000}".into())]],
    );
}

#[test]
fn replacement_publication_and_late_failures_remain_complete_query_errors() {
    for sql in [
        "SELECT Replace(ChrW(55296),'a','b')",
        "SELECT Replace('😀',ChrW(55357),'')",
    ] {
        assert!(matches!(
            run(sql),
            Err(QueryError::Evaluation(EvalError::UnrepresentableText))
        ));
    }
    assert!(matches!(
        run("SELECT LenB(Replace(ChrW(55296),'a','b')),1/0"),
        Err(QueryError::Evaluation(EvalError::DivisionByZero))
    ));
}

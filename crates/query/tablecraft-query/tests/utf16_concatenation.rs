use tablecraft_core::values::Value;
use tablecraft_expressions::EvalError;
use tablecraft_query::{QueryError, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

#[test]
fn raw_concatenation_rebinds_parameters_and_publishes_only_complete_results() {
    let source = MemorySource::default();
    let parsed = parse_query(
        "PARAMETERS H LONG,L LONG; SELECT ChrW(H)&ChrW(L),Len(ChrW(H)&NULL),VarType(CVar(ChrW(H)&''))",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    for (high, low, text) in [(55357, 56832, "😀"), (55358, 56704, "🦀")] {
        assert_eq!(
            plan.execute(
                &source,
                &[("H", Value::Int32(high)), ("L", Value::Int32(low))],
                Default::default()
            )
            .unwrap()
            .rows(),
            &[vec![
                Value::Text(text.into()),
                Value::Int32(1),
                Value::Int16(8)
            ]]
        );
    }

    for sql in [
        "SELECT 1,ChrW(55296)&'x'",
        "SELECT Len(S) FROM (SELECT ChrW(55296)&'' AS S) AS Q",
        "SELECT LenB(ChrB(65)&ChrB(0))",
    ] {
        let parsed = parse_query(sql, Default::default()).unwrap();
        assert_eq!(
            prepare(parsed.query(), &source)
                .unwrap()
                .execute(&source, &[], Default::default())
                .err(),
            Some(QueryError::Evaluation(EvalError::UnrepresentableText)),
            "{sql}"
        );
    }
}

#[test]
fn grouped_and_correlated_concatenations_can_consume_raw_units() {
    let source = MemorySource::default();
    for (sql, expected) in [
        (
            "SELECT Len(Left(T,1)&NULL),Sum(AscW(Right(T,1)&'')) FROM (SELECT '😀' AS T UNION ALL SELECT '😁') AS Q GROUP BY Len(Left(T,1)&NULL)",
            vec![vec![Value::Int32(1), Value::Int64(-17407)]],
        ),
        (
            "SELECT T,(SELECT Left(Q.T,1)&Right(Q.T,1)) FROM (SELECT '😀' AS T UNION ALL SELECT '🦀') AS Q",
            vec![
                vec![Value::Text("😀".into()), Value::Text("😀".into())],
                vec![Value::Text("🦀".into()), Value::Text("🦀".into())],
            ],
        ),
        (
            "SELECT Nz(NULL)&'x',NULL&Nz(NULL),NULL&NULL",
            vec![vec![
                Value::Text("x".into()),
                Value::Text(String::new()),
                Value::Null,
            ]],
        ),
    ] {
        let parsed = parse_query(sql, Default::default()).unwrap();
        let result = prepare(parsed.query(), &source)
            .unwrap()
            .execute(&source, &[], Default::default())
            .unwrap();
        assert_eq!(result.rows(), expected, "{sql}");
    }
}

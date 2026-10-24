use tablecraft_core::values::Value;
use tablecraft_expressions::EvalError;
use tablecraft_query::{QueryError, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

#[test]
fn string_addition_rebinds_raw_operands_without_publishing_them_early() {
    let source = MemorySource::default();
    let parsed = parse_query(
        "PARAMETERS H LONG,L LONG; SELECT ChrW(H)+ChrW(L),ChrW(H)+NULL,Len(ChrW(H)+'x'),VarType(ChrW(H)+'')",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    for (high, low, text) in [(55357, 56832, "😀"), (55358, 56704, "🦀")] {
        let result = plan
            .execute(
                &source,
                &[("H", Value::Int32(high)), ("L", Value::Int32(low))],
                Default::default(),
            )
            .unwrap();
        assert_eq!(
            result.rows(),
            &[vec![
                Value::Text(text.into()),
                Value::Null,
                Value::Int32(2),
                Value::Int16(8)
            ]]
        );
    }

    for sql in ["SELECT 1,ChrW(55296)+'x'", "SELECT LenB(ChrB(65)+ChrB(0))"] {
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
fn raw_string_addition_composes_in_groups_and_correlated_queries() {
    let source = MemorySource::default();
    for (sql, expected) in [
        (
            "SELECT Len(Left(T,1)+''),Sum(AscW(Right(T,1)+'')) FROM (SELECT '😀' AS T UNION ALL SELECT '😁') AS Q GROUP BY Len(Left(T,1)+'')",
            vec![vec![Value::Int32(1), Value::Int64(-17407)]],
        ),
        (
            "SELECT T,(SELECT Left(Q.T,1)+Right(Q.T,1)) FROM (SELECT '😀' AS T UNION ALL SELECT '🦀') AS Q",
            vec![
                vec![Value::Text("😀".into()), Value::Text("😀".into())],
                vec![Value::Text("🦀".into()), Value::Text("🦀".into())],
            ],
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

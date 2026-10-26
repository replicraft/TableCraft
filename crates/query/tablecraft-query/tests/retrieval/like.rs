use super::*;
use tablecraft_expressions::predicates::LikeMode;
fn texts() -> MemorySource {
    MemorySource::new(vec![table(
        "Words",
        &[("ID", ScalarType::Int64), ("Label", ScalarType::Text)],
        vec![
            vec![Value::Int64(1), Value::Text("apple".into())],
            vec![Value::Int64(2), Value::Text("ab".into())],
            vec![Value::Int64(3), Value::Text("b2".into())],
            vec![Value::Int64(4), Value::Text("".into())],
            vec![Value::Int64(5), Value::Null],
        ],
    )])
    .unwrap()
}
fn execute(
    sql: &str,
    source: &dyn Source,
    params: &[(&str, Value)],
    mode: Option<LikeMode>,
) -> Result<QueryResult, QueryError> {
    prepare(
        parse_query(sql, ParseLimits::default()).unwrap().query(),
        source,
    )?
    .execute(
        source,
        params,
        ExecutionOptions {
            like_mode: mode,
            collation: Some(&Ordinal),
            ..Default::default()
        },
    )
}
#[test]
fn like_where_projection_and_three_valued_negation_use_explicit_modes() {
    let source = texts();
    for (mode, pattern) in [(LikeMode::Ansi89, "a*"), (LikeMode::Ansi92, "a%")] {
        let query = format!(
            "SELECT ID, Label NOT LIKE '{pattern}' AS Rejected FROM Words WHERE Label LIKE '{pattern}' ORDER BY ID"
        );
        assert_eq!(
            execute(&query, &source, &[], Some(mode)).unwrap().rows(),
            &[
                vec![Value::Int64(1), Value::Boolean(false)],
                vec![Value::Int64(2), Value::Boolean(false)]
            ]
        );
    }
    let result = execute(
        "SELECT Label LIKE '*' AS Matches, Label NOT LIKE '*' AS Rejected FROM Words WHERE ID=5",
        &source,
        &[],
        Some(LikeMode::Ansi89),
    )
    .unwrap();
    assert_eq!(result.rows(), &[vec![Value::Null, Value::Null]]);
}
#[test]
fn parameter_patterns_are_rebound_for_reused_prepared_plans() {
    let source = texts();
    let query = parse_query(
        "PARAMETERS P TEXT(20); SELECT ID FROM Words WHERE Label LIKE P ORDER BY ID",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    for (pattern, expected) in [
        ("a*", ints(&[&[Some(1)], &[Some(2)]])),
        ("?#", ints(&[&[Some(3)]])),
        ("[]", ints(&[&[Some(4)]])),
    ] {
        let result = plan
            .execute(
                &source,
                &[("P", Value::Text(pattern.into()))],
                ExecutionOptions {
                    like_mode: Some(LikeMode::Ansi89),
                    collation: Some(&Ordinal),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(result.rows(), expected);
    }
}
#[test]
fn grouping_aggregate_traversal_and_nested_like_remain_correct() {
    let source = texts();
    let result = execute(
        "SELECT MIN(Label) LIKE '[]' AS IsEmpty FROM Words",
        &source,
        &[],
        Some(LikeMode::Ansi89),
    )
    .unwrap();
    assert_eq!(result.rows(), &[vec![Value::Boolean(true)]]);
    let result = execute(
        "SELECT Label, Count(*) FROM Words GROUP BY Label HAVING Label LIKE 'a*' ORDER BY Label",
        &source,
        &[],
        Some(LikeMode::Ansi89),
    )
    .unwrap();
    assert_eq!(result.rows().len(), 2);
    assert!(matches!(
        execute(
            "SELECT Label LIKE '*', Count(*) FROM Words",
            &source,
            &[],
            Some(LikeMode::Ansi89)
        ),
        Err(QueryError::Grouping)
    ));
    assert_eq!(
        execute(
            "SELECT ID FROM Words AS w WHERE EXISTS (SELECT 1 WHERE w.Label LIKE '?#') ORDER BY ID",
            &source,
            &[],
            Some(LikeMode::Ansi89)
        )
        .unwrap()
        .rows(),
        ints(&[&[Some(3)]])
    );
}
#[test]
fn missing_policies_bad_patterns_and_eager_errors_are_not_silently_ignored() {
    let source = texts();
    for (sql, expected) in [
        (
            "SELECT Label LIKE '[' FROM Words",
            EvalError::InvalidPattern,
        ),
        (
            "SELECT ID LIKE '*' FROM Words",
            EvalError::NumberFormatRequired,
        ),
        (
            "SELECT FALSE AND ('a' LIKE '[') FROM Words",
            EvalError::InvalidPattern,
        ),
        (
            "SELECT NULL LIKE (1/0) FROM Words",
            EvalError::DivisionByZero,
        ),
    ] {
        assert_eq!(
            execute(sql, &source, &[], Some(LikeMode::Ansi89)).err(),
            Some(QueryError::Evaluation(expected)),
            "{sql}"
        );
    }
    assert_eq!(
        execute("SELECT Label LIKE '*' FROM Words", &source, &[], None).err(),
        Some(QueryError::Evaluation(EvalError::LikeModeRequired))
    );
}

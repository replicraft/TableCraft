//! Deferred default admission keeps projection separate from text operations.
use super::*;

#[test]
fn unknown_default_preserves_projection_and_independently_supported_field_policies() {
    for field in [[9, 4, 0, 0], [31, 4, 0, 1]] {
        let mut bytes = copy();
        orders(&mut bytes, [31, 4, 0, 1], field);
        let comparisons = RefCell::new(budget());
        let (source, resolver) = open_source(bytes)
            .with_deferred_database_collation_resolver(&comparisons)
            .unwrap();

        for (sql, expected) in [
            (
                "SELECT TOP 1 ORDR_NBR+1 FROM ROULETTE_TB ORDER BY ORDR_NBR",
                vec![Value::Int64(2)],
            ),
            (
                "SELECT TOP 1 NBR_VALUE FROM ROULETTE_TB ORDER BY ORDR_NBR",
                vec![Value::Text("2".into())],
            ),
        ] {
            let query = parse_query(sql, ParseLimits::default()).unwrap();
            let result = prepare(query.query(), &source)
                .unwrap()
                .execute(
                    &source,
                    &[],
                    ExecutionOptions {
                        collation: Some(resolver.default_collation()),
                        collation_resolver: Some(&resolver),
                        ..Default::default()
                    },
                )
                .unwrap();
            assert_eq!(result.rows(), [expected]);
        }

        let query = parse_query(
            "SELECT TOP 1 NBR_VALUE='2' FROM ROULETTE_TB ORDER BY ORDR_NBR",
            ParseLimits::default(),
        )
        .unwrap();
        let result = prepare(query.query(), &source).unwrap().execute(
            &source,
            &[],
            ExecutionOptions {
                collation: Some(resolver.default_collation()),
                collation_resolver: Some(&resolver),
                ..Default::default()
            },
        );
        if field == [9, 4, 0, 0] {
            assert_eq!(result.unwrap().rows(), [vec![Value::Boolean(true)]]);
        } else {
            assert!(matches!(
                result,
                Err(QueryError::Evaluation(EvalError::CollationRequired))
            ));
        }

        let query = parse_query("SELECT 'a'='A'", ParseLimits::default()).unwrap();
        assert!(matches!(
            prepare(query.query(), &source).unwrap().execute(
                &source,
                &[],
                ExecutionOptions {
                    collation: Some(resolver.default_collation()),
                    collation_resolver: Some(&resolver),
                    ..Default::default()
                },
            ),
            Err(QueryError::Evaluation(EvalError::CollationRequired))
        ));
    }
}

#[test]
fn deferred_selection_still_refuses_after_schema_discovery() {
    let source = open_source(copy());
    source.columns("ROULETTE_TB").unwrap();
    let comparisons = RefCell::new(budget());
    assert!(matches!(
        source.with_deferred_database_collation_resolver(&comparisons),
        Err(InspectError::Unsupported)
    ));
}

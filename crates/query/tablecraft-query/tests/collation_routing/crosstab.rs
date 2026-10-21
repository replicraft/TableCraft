use super::support::*;
#[test]
fn dynamic_and_fixed_pivots_and_row_groups_use_their_own_field_policies() {
    let source = Fixture::new();
    let resolver = Resolver::new();
    for sql in [
        "TRANSFORM Count(*) SELECT 1 AS G FROM T GROUP BY 1 PIVOT A",
        "TRANSFORM Count(*) SELECT 1 AS G FROM T GROUP BY 1 PIVOT A IN ('a','b')",
    ] {
        assert_eq!(
            run(sql, &source, Some(&resolver)).unwrap(),
            [vec![Value::Int64(1), Value::Int64(2), Value::Int64(2)]],
            "{sql}"
        );
    }
    // Value ordering and the ASCII-insensitive heading-name namespace remain
    // separate: distinct a/A values cannot publish colliding column names.
    assert_eq!(
        run(
            "TRANSFORM Count(*) SELECT 1 AS G FROM T GROUP BY 1 PIVOT B",
            &source,
            Some(&resolver)
        )
        .err(),
        Some(QueryError::AmbiguousName)
    );
    let mut reverse_source = Fixture::new();
    reverse_source.policies[3] = Some(TextCollationId::from_host(2));
    let parsed = parse_query(
        "TRANSFORM Count(*) SELECT 1 AS G FROM T GROUP BY 1 PIVOT K",
        ParseLimits::default(),
    )
    .unwrap();
    let result = prepare(parsed.query(), &reverse_source)
        .unwrap()
        .execute(
            &reverse_source,
            &[],
            ExecutionOptions {
                collation: Some(&DEFAULT),
                collation_resolver: Some(&resolver),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(result.columns(), ["G", "d", "c", "b", "a"]);
    let sql = "TRANSFORM First(A) SELECT B FROM T GROUP BY B PIVOT Id IN (1,2,3,4)";
    assert_eq!(run(sql, &source, Some(&resolver)).unwrap().len(), 3);
    let parsed = parse_query(
        "TRANSFORM Count(*) SELECT 1 AS G FROM T GROUP BY 1 PIVOT A IN ('a','A')",
        ParseLimits::default(),
    )
    .unwrap();
    assert_eq!(
        prepare(parsed.query(), &source)
            .unwrap()
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    collation: Some(&DEFAULT),
                    collation_resolver: Some(&resolver),
                    ..Default::default()
                }
            )
            .err(),
        Some(QueryError::AmbiguousName)
    );
}

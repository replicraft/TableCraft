use super::support::*;
use tablecraft_core::values::ExpressionValue;
use tablecraft_query::execute_with_schema_discovery;
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

#[test]
fn discovered_named_cells_keep_their_field_policy_for_outer_comparisons() {
    let mut source = Fixture::new();
    source.named = parse_query(
        "TRANSFORM First(A) SELECT 1 AS G FROM T GROUP BY 1 PIVOT Id",
        ParseLimits::default(),
    )
    .unwrap()
    .into_query();
    let resolver = Resolver::new();
    let query = parse_query(
        "SELECT [2] FROM Named WHERE [2]='a'",
        ParseLimits::default(),
    )
    .unwrap();
    let result = execute_with_schema_discovery(
        query.query(),
        &source,
        &[],
        ExecutionOptions {
            collation: Some(&DEFAULT),
            collation_resolver: Some(&resolver),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        result.rows(),
        [vec![ExpressionValue::Value(Value::Text("A".into()))]]
    );
    assert_eq!(source.scans.get(), 1);
    assert!(!resolver.hits.borrow().is_empty());
    assert!(resolver.hits.borrow().iter().all(|id| *id == 1));
}

#[test]
fn runtime_domain_discovery_retains_field_policies_in_criteria() {
    let mut source = Fixture::new();
    source.named = parse_query(
        "TRANSFORM First(A) SELECT 1 AS G FROM T GROUP BY 1 PIVOT Id",
        ParseLimits::default(),
    )
    .unwrap()
    .into_query();
    let resolver = Resolver::new();
    let query = parse_query(
        r#"SELECT DCount('*','Named',"[2]='a'")"#,
        ParseLimits::default(),
    )
    .unwrap();
    let result = execute_with_schema_discovery(
        query.query(),
        &source,
        &[],
        ExecutionOptions {
            collation: Some(&DEFAULT),
            collation_resolver: Some(&resolver),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        result.rows(),
        [vec![ExpressionValue::Value(Value::Int64(1))]]
    );
    assert_eq!(source.scans.get(), 1);
    assert!(!resolver.hits.borrow().is_empty());
    assert!(resolver.hits.borrow().iter().all(|id| *id == 1));
}

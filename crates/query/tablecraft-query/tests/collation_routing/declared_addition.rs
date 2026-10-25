use super::support::*;
use tablecraft_expressions::functions::conversions::text::NumberFormat;

#[test]
fn declared_string_results_retain_policies_through_derived_and_set_sources() {
    let source = Fixture::new();
    let resolver = Resolver::new();
    for sql in [
        "SELECT Count(*) FROM T WHERE A+CVar(CInt(0))='A0'",
        "SELECT Count(*) FROM (SELECT A+CVar(CInt(0)) AS S FROM T) AS Q WHERE S='A0'",
        "SELECT Count(*) FROM (SELECT A+CVar(CInt(0)) AS S FROM T UNION ALL SELECT 'unused' WHERE False) AS Q WHERE S='A0'",
        "SELECT Count(*) FROM Named WHERE A+CVar(CInt(0))='A0'",
    ] {
        let parsed = parse_query(sql, ParseLimits::default()).unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        let result = plan
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    declared_integral: true,
                    number_format: Some(NumberFormat::Invariant),
                    collation_resolver: Some(&resolver),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(result.rows(), [vec![Value::Int64(2)]], "{sql}");
    }
    assert!(resolver.hits.borrow().contains(&1));

    let parsed = parse_query(
        "SELECT A+CVar(CInt(0))=B+CVar(CInt(0)) FROM T",
        ParseLimits::default(),
    )
    .unwrap();
    let result = prepare(parsed.query(), &source).unwrap().execute(
        &source,
        &[],
        ExecutionOptions {
            declared_integral: true,
            number_format: Some(NumberFormat::Invariant),
            collation_resolver: Some(&resolver),
            ..Default::default()
        },
    );
    assert!(matches!(result, Err(QueryError::CollationConflict)));
}

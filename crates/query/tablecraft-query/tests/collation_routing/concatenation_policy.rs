use super::{raw_support::RawResolver, support::*};
use tablecraft_expressions::functions::{
    conversions::text::NumberFormat, text::cstr::DateTextFormat,
};

#[test]
fn formatted_non_string_peers_cannot_contribute_latent_field_policies() {
    let source = Fixture::new();
    let resolver = Resolver::new();
    for operator in ["&", "+"] {
        for reverse in [false, true] {
            let (left, right) = if reverse {
                ("IIf(True,2,B)", "A")
            } else {
                ("A", "IIf(True,2,B)")
            };
            let expression = format!("{left}{operator}{right}");
            for sql in [
                format!("SELECT {expression}=CStr({left})&CStr({right}) FROM T"),
                format!("SELECT S=S FROM (SELECT {expression} AS S FROM T) Q"),
            ] {
                let parsed = parse_query(&sql, Default::default()).unwrap();
                let result = prepare(parsed.query(), &source)
                    .unwrap()
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
                assert!(
                    result
                        .rows()
                        .iter()
                        .all(|row| row == &[Value::Boolean(true)]),
                    "{sql}"
                );
            }
        }
    }
}

#[test]
fn actual_strings_and_null_field_lineage_remain_conservative() {
    let source = Fixture::new();
    let resolver = Resolver::new();
    for expression in ["A&IIf(False,2,B)", "A+IIf(False,2,B)", "A&IIf(True,NULL,B)"] {
        let parsed = parse_query(
            &format!("SELECT ({expression})='a' FROM T"),
            Default::default(),
        )
        .unwrap();
        assert_eq!(
            prepare(parsed.query(), &source)
                .unwrap()
                .execute(
                    &source,
                    &[],
                    ExecutionOptions {
                        declared_integral: true,
                        number_format: Some(NumberFormat::Invariant),
                        collation_resolver: Some(&resolver),
                        ..Default::default()
                    }
                )
                .err(),
            Some(QueryError::CollationConflict),
            "{expression}"
        );
    }
}

#[test]
fn scalar_string_variant_dispatch_ignores_complete_numeric_peer_lineage() {
    let source = Fixture::new();
    let resolver = Resolver::new();
    for predicate in ["'2'=IIf(True,2,A&B)", "IIf(True,2,A&B)='2'"] {
        let syntax =
            parse_query(&format!("SELECT {predicate} FROM T"), Default::default()).unwrap();
        let result = prepare(syntax.query(), &source)
            .unwrap()
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    declared_integral: true,
                    number_format: Some(NumberFormat::Invariant),
                    collation: Some(&DEFAULT),
                    collation_resolver: Some(&resolver),
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(
            result
                .rows()
                .iter()
                .all(|row| row == &[Value::Boolean(true)])
        );
        assert!(resolver.hits.borrow().is_empty());
    }
}

#[test]
fn boolean_date_and_raw_construction_keep_conversion_and_provider_boundaries() {
    let source = Fixture::new();
    let resolver = Resolver::new();
    for (peer, spelling) in [("True", "True"), ("CDate(0)", "00:00:00")] {
        let sql = format!("SELECT (A&IIf(True,{peer},B))=(A&'{spelling}') FROM T");
        let syntax = parse_query(&sql, Default::default()).unwrap();
        let result = prepare(syntax.query(), &source)
            .unwrap()
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    number_format: Some(NumberFormat::Invariant),
                    date_format: Some(DateTextFormat::Iso),
                    collation_resolver: Some(&resolver),
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(
            result
                .rows()
                .iter()
                .all(|row| row == &[Value::Boolean(true)]),
            "{sql}: {:?}",
            result.rows()
        );
    }
    let syntax = parse_query(
        "SELECT (A&ChrW(55296)&IIf(True,2,B))=(A&ChrW(55296)&'2') FROM T",
        Default::default(),
    )
    .unwrap();
    let result = prepare(syntax.query(), &source)
        .unwrap()
        .execute(
            &source,
            &[],
            ExecutionOptions {
                number_format: Some(NumberFormat::Invariant),
                collation_resolver: Some(&RawResolver),
                ..Default::default()
            },
        )
        .unwrap();
    assert!(
        result
            .rows()
            .iter()
            .all(|row| row == &[Value::Boolean(true)])
    );
}

#[test]
fn fixed_heading_policy_validation_only_defers_when_runtime_construction_can_change_it() {
    let source = Fixture::new();
    let resolver = Resolver::new();
    for (pivot, headings, scanned) in [
        ("A", "'a','A'", false),
        ("A&IIf(True,0,B)", "'a0','A0'", true),
    ] {
        source.scans.set(0);
        let syntax = parse_query(
            &format!(
                "TRANSFORM Count(*) SELECT 1 AS G FROM T GROUP BY 1 PIVOT {pivot} IN ({headings})"
            ),
            Default::default(),
        )
        .unwrap();
        let result = prepare(syntax.query(), &source).unwrap().execute(
            &source,
            &[],
            ExecutionOptions {
                number_format: Some(NumberFormat::Invariant),
                collation_resolver: Some(&resolver),
                ..Default::default()
            },
        );
        assert_eq!(result.err(), Some(QueryError::AmbiguousName), "{pivot}");
        assert_eq!(source.scans.get() > 0, scanned, "{pivot}");
    }
}

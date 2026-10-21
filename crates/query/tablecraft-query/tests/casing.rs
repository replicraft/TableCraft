use tablecraft_core::values::Value;
use tablecraft_expressions::{
    EvalError, comparison::Collation, functions::text::casing::CaseLocale,
};
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::{ParseLimits, parse_query};
struct BinaryCollation;
impl Collation for BinaryCollation {
    fn compare(&self, left: &str, right: &str) -> Result<std::cmp::Ordering, EvalError> {
        Ok(left.encode_utf16().cmp(right.encode_utf16()))
    }
}
#[test]
fn casing_policies_remain_execution_local_after_binding_and_failure() {
    let source = MemorySource::default();
    let query = parse_query(
        "PARAMETERS S TEXT; SELECT LCase(S),UCase(S)",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    for (locale, expected) in [
        (Some("und"), Some(("i", "I"))),
        (Some("tr"), Some(("ı", "I"))),
        (None, None),
        (Some("en-US"), Some(("i", "I"))),
    ] {
        let locale = locale.map(|text| text.parse::<CaseLocale>().unwrap());
        let result = plan.execute(
            &source,
            &[("S", Value::Text("I".into()))],
            ExecutionOptions {
                case_locale: locale.as_ref(),
                ..Default::default()
            },
        );
        if let Some((lower, upper)) = expected {
            assert_eq!(
                result.unwrap().rows(),
                [vec![Value::Text(lower.into()), Value::Text(upper.into())]]
            );
        } else {
            assert_eq!(
                result.err(),
                Some(QueryError::Evaluation(EvalError::CaseLocaleRequired))
            );
        }
    }
}
#[test]
fn casing_fields_grouping_aggregate_correlation_and_unused_errors_survive() {
    let source = MemorySource::default();
    let locale = "und".parse().unwrap();
    for (sql, expected) in [
        (
            "SELECT LCase(A.S),Count(*) FROM (SELECT 'AbC' AS S) AS A GROUP BY LCase(A.S)",
            vec![Value::Text("abc".into()), Value::Int64(1)],
        ),
        (
            "SELECT (SELECT UCase(A.S)) FROM (SELECT 'straße' AS S) AS A",
            vec![Value::Text("STRASSE".into())],
        ),
        (
            "SELECT LCase(Max(A.S)) FROM (SELECT 'AbC' AS S) AS A",
            vec![Value::Text("abc".into())],
        ),
    ] {
        let query = parse_query(sql, ParseLimits::default()).unwrap();
        assert_eq!(
            prepare(query.query(), &source)
                .unwrap()
                .execute(
                    &source,
                    &[],
                    ExecutionOptions {
                        case_locale: Some(&locale),
                        collation: Some(&BinaryCollation),
                        ..Default::default()
                    }
                )
                .unwrap()
                .rows(),
            [expected]
        );
    }
    for sql in [
        "SELECT LCase(A.S) FROM (SELECT 'A' AS S) AS A GROUP BY UCase(A.S)",
        "SELECT A.X FROM (SELECT 1 AS X,UCase(1/0) AS Unused) AS A",
    ] {
        let query = parse_query(sql, ParseLimits::default()).unwrap();
        assert!(
            prepare(query.query(), &source)
                .and_then(|plan| plan.execute(
                    &source,
                    &[],
                    ExecutionOptions {
                        case_locale: Some(&locale),
                        ..Default::default()
                    }
                ))
                .is_err()
        );
    }
}

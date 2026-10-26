use super::{raw_support::Units, support::*};
use tablecraft_expressions::{
    EvalError,
    functions::{conversions::text::NumberFormat, text::cstr::DateTextFormat},
    predicates::LikeMode,
};

fn options(declared: bool) -> ExecutionOptions<'static> {
    ExecutionOptions {
        declared_integral: declared,
        number_format: Some(NumberFormat::Invariant),
        date_format: Some(DateTextFormat::Iso),
        like_mode: Some(LikeMode::Ansi89),
        collation: Some(&DEFAULT),
        ..Default::default()
    }
}

#[test]
fn numeric_boolean_and_date_like_operands_match_explicit_string_conversion() {
    let source = Fixture::new();
    for declared in [false, true] {
        for (value, pattern) in [
            ("CByte(12)", "'1*'"),
            ("CInt(12)", "CLng(12)"),
            ("CLngLng(12)", "'12'"),
            ("CSng(1.5)", "'1.5'"),
            ("CDbl(1.5)", "'1.5'"),
            ("CCur(1.25)", "'1.25'"),
            ("CDec(1.25)", "'1.25'"),
            ("True", "'T*'"),
            ("#2000-01-01#", "'2000*'"),
        ] {
            let sql = format!(
                "SELECT {value} LIKE {pattern}, CStr({value}) LIKE CStr({pattern}), {value} NOT LIKE {pattern}"
            );
            let parsed = parse_query(&sql, Default::default()).unwrap();
            let result = prepare(parsed.query(), &source)
                .unwrap()
                .execute(&source, &[], options(declared))
                .unwrap();
            assert_eq!(
                result.rows(),
                &[vec![
                    Value::Boolean(true),
                    Value::Boolean(true),
                    Value::Boolean(false)
                ]],
                "{declared}: {sql}"
            );
        }
    }
}

#[test]
fn converted_operands_discard_only_their_latent_string_policies() {
    let source = Fixture::new();
    for declared in [false, true] {
        for (sql, expected_hits) in [
            (
                "SELECT IIf(True,12,B) LIKE IIf(True,'1*',A) FROM T WHERE Id=1",
                vec![1],
            ),
            (
                "SELECT IIf(True,12,A) LIKE IIf(True,12,B) FROM T WHERE Id=1",
                vec![],
            ),
            (
                "SELECT IIf(True,'12',A) LIKE IIf(True,12,B) FROM T WHERE Id=1",
                vec![1],
            ),
        ] {
            let resolver = Resolver::new();
            let parsed = parse_query(sql, Default::default()).unwrap();
            let plan = prepare(parsed.query(), &source).unwrap();
            assert!(resolver.hits.borrow().is_empty());
            let result = plan
                .execute(
                    &source,
                    &[],
                    ExecutionOptions {
                        collation_resolver: Some(&resolver),
                        ..options(declared)
                    },
                )
                .unwrap();
            assert_eq!(result.rows(), &[vec![Value::Boolean(true)]], "{sql}");
            assert_eq!(*resolver.hits.borrow(), expected_hits, "{sql}");
        }
    }
}

#[test]
fn conversion_and_null_admission_precede_comparison_provider_resolution() {
    let source = Fixture::new();
    for declared in [false, true] {
        for (sql, expected) in [
            (
                "SELECT 12 LIKE U FROM T WHERE Id=1",
                Some(EvalError::NumberFormatRequired),
            ),
            (
                "SELECT #2000-01-01# LIKE U FROM T WHERE Id=1",
                Some(EvalError::DateFormatRequired),
            ),
            ("SELECT NULL LIKE U FROM T WHERE Id=1", None),
            ("SELECT U NOT LIKE NULL FROM T WHERE Id=1", None),
            ("SELECT ChrB(65) LIKE NULL", None),
        ] {
            let resolver = Resolver::new();
            let parsed = parse_query(sql, Default::default()).unwrap();
            let result = prepare(parsed.query(), &source).unwrap().execute(
                &source,
                &[],
                ExecutionOptions {
                    number_format: None,
                    date_format: None,
                    collation_resolver: Some(&resolver),
                    ..options(declared)
                },
            );
            match expected {
                Some(error) => {
                    assert_eq!(result.err(), Some(QueryError::Evaluation(error)), "{sql}")
                }
                None => assert_eq!(result.unwrap().rows(), &[vec![Value::Null]], "{sql}"),
            }
            assert!(resolver.hits.borrow().is_empty(), "{sql}");
        }
    }
}

#[test]
fn eager_operands_and_actual_string_conflicts_remain_observable() {
    let source = Fixture::new();
    for declared in [false, true] {
        for (sql, expected) in [
            (
                "SELECT NULL LIKE (1/0)",
                QueryError::Evaluation(EvalError::DivisionByZero),
            ),
            (
                "SELECT (1/0) LIKE NULL",
                QueryError::Evaluation(EvalError::DivisionByZero),
            ),
            ("SELECT A LIKE B FROM T", QueryError::CollationConflict),
            (
                "SELECT CVErr(7) LIKE '*'",
                QueryError::Evaluation(EvalError::TypeMismatch),
            ),
        ] {
            let resolver = Resolver::new();
            let parsed = parse_query(sql, Default::default()).unwrap();
            let result = prepare(parsed.query(), &source).unwrap().execute(
                &source,
                &[],
                ExecutionOptions {
                    collation_resolver: Some(&resolver),
                    ..options(declared)
                },
            );
            assert_eq!(result.err(), Some(expected), "{sql}");
            assert!(resolver.hits.borrow().is_empty(), "{sql}");
        }
    }
}

#[test]
fn converted_peers_preserve_borrowed_raw_unit_matching() {
    let source = Fixture::new();
    for declared in [false, true] {
        let parsed = parse_query(
            "SELECT ChrW(49) LIKE 1, 1 LIKE ChrW(49), ChrW(55296) LIKE '?'",
            Default::default(),
        )
        .unwrap();
        let result = prepare(parsed.query(), &source)
            .unwrap()
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    collation: Some(&Units(false)),
                    ..options(declared)
                },
            )
            .unwrap();
        assert_eq!(result.rows(), &[vec![Value::Boolean(true); 3]]);
    }
}

#[test]
fn cancellation_from_converted_like_provider_does_not_publish_a_result() {
    let source = Fixture::new();
    let resolver = Resolver::new();
    resolver.cancel_on_resolve.set(true);
    let cancelled = || resolver.cancelled.get();
    let parsed = parse_query("SELECT 12 LIKE A FROM T WHERE Id=1", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let result = plan.execute(
        &source,
        &[],
        ExecutionOptions {
            collation_resolver: Some(&resolver),
            cancelled: Some(&cancelled),
            ..options(false)
        },
    );
    assert!(matches!(
        result,
        Err(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
    ));
    assert_eq!(*resolver.hits.borrow(), vec![1]);
}

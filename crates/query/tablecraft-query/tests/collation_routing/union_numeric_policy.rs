use super::support::*;
use std::cell::Cell;
use tablecraft_core::values::Double;
use tablecraft_expressions::{EvalError, functions::conversions::text::NumberFormat};
use tablecraft_query::QueryLimits;
use tablecraft_schema::{Column, ScalarType};

fn execute(
    sql: &str,
    source: &Fixture,
    resolver: &Resolver,
    options: ExecutionOptions<'_>,
) -> Result<Vec<Vec<Value>>, QueryError> {
    let syntax = parse_query(sql, Default::default()).unwrap();
    prepare(syntax.query(), source)?
        .execute(
            source,
            &[],
            ExecutionOptions {
                collation: Some(&DEFAULT),
                collation_resolver: Some(resolver),
                ..options
            },
        )
        .map(|result| result.rows().to_vec())
}

fn options(declared: bool) -> ExecutionOptions<'static> {
    ExecutionOptions {
        number_format: Some(NumberFormat::Invariant),
        declared_integral: declared,
        ..Default::default()
    }
}

#[test]
fn all_numeric_text_promotions_discard_only_the_numeric_sources_latent_policy() {
    let source = Fixture::new();
    for (numeric, text) in [
        ("CBool(True)", "True"),
        ("CByte(2)", "2"),
        ("CInt(2)", "2"),
        ("CLng(2)", "2"),
        ("CLngLng(2)", "2"),
        ("CSng(1.5)", "1.5"),
        ("CDbl(1.5)", "1.5"),
        ("CCur(1.25)", "1.25"),
        ("CDec(1.25)", "1.25"),
    ] {
        for declared in [false, true] {
            for all in ["UNION", "UNION ALL"] {
                for reverse in [false, true] {
                    let string_arm = "SELECT A AS V FROM T WHERE Id=1";
                    let numeric_arm =
                        format!("SELECT IIf(True,{numeric},B) AS V FROM T WHERE Id=1");
                    let arms = if reverse {
                        format!("{numeric_arm} {all} {string_arm}")
                    } else {
                        format!("{string_arm} {all} {numeric_arm}")
                    };
                    let sql = format!("{arms} ORDER BY V");
                    let resolver = Resolver::new();
                    let rows = execute(&sql, &source, &resolver, options(declared)).unwrap();
                    let mut expected = ["a", text];
                    expected.sort_by_key(|value| value.to_ascii_lowercase());
                    assert_eq!(rows, texts(&expected), "{sql}");
                    assert!(!resolver.hits.borrow().is_empty());
                    assert!(resolver.hits.borrow().iter().all(|&key| key == 1));
                }
            }
        }
    }
}

#[test]
fn nested_arm_grouping_and_named_outputs_retain_actual_string_contributions() {
    let mut source = Fixture::new();
    source.named = parse_query(
        "SELECT A AS V FROM T WHERE Id<=2 UNION ALL SELECT IIf(True,2,B) FROM T WHERE Id=1",
        Default::default(),
    )
    .unwrap()
    .into_query();
    for sql in [
        "SELECT V,Count(*) FROM Named GROUP BY V",
        "SELECT V,Count(*) FROM (SELECT V FROM Named UNION ALL SELECT A FROM T WHERE Id=1) Q GROUP BY V",
    ] {
        let resolver = Resolver::new();
        let rows = execute(sql, &source, &resolver, options(false)).unwrap();
        let count = if sql.contains("FROM (") { 3 } else { 2 };
        assert_eq!(
            rows,
            [
                vec![Value::Text("a".into()), Value::Int64(count)],
                vec![Value::Text("2".into()), Value::Int64(1)]
            ]
        );
        assert!(resolver.hits.borrow().iter().all(|&key| key == 1));
    }

    let sql = "SELECT V FROM Named UNION ALL SELECT B FROM T WHERE Id=1 ORDER BY V";
    assert_eq!(
        execute(sql, &source, &Resolver::new(), options(false)),
        Err(QueryError::CollationConflict)
    );
}

#[test]
fn actual_text_and_empty_text_arms_keep_their_existing_policy_authority() {
    let source = Fixture::new();
    for sql in [
        "SELECT A AS V FROM T WHERE Id=1 UNION ALL SELECT IIf(False,2,B) FROM T WHERE Id=1 ORDER BY V",
        "SELECT A AS V FROM T WHERE Id<=2 UNION ALL SELECT IIf(True,2,B) FROM T WHERE Id=1 UNION ALL SELECT B FROM T WHERE False ORDER BY V",
        "SELECT A AS V FROM T WHERE Id<=2 UNION ALL SELECT IIf(True,2,B) FROM T WHERE False ORDER BY V",
    ] {
        assert_eq!(
            execute(sql, &source, &Resolver::new(), options(false)),
            Err(QueryError::CollationConflict),
            "{sql}"
        );
    }
}

#[test]
fn null_only_and_mixed_null_numeric_arms_keep_conservative_null_policies() {
    let source = Fixture::new();
    for expression in ["IIf(True,NULL,B)", "IIf(True,IIf(Id=1,2,NULL),B)"] {
        let sql = format!(
            "SELECT A AS V FROM T WHERE Id<=2 UNION ALL SELECT {expression} FROM T WHERE Id<=2 ORDER BY V"
        );
        assert_eq!(
            execute(&sql, &source, &Resolver::new(), options(false)),
            Err(QueryError::CollationConflict),
            "{sql}"
        );
    }
}

#[test]
fn rendering_and_eager_errors_fail_before_any_comparison_or_completed_result() {
    let source = Fixture::new();
    for (expression, error) in [
        ("IIf(True,2,B)", EvalError::NumberFormatRequired),
        ("IIf(True,1/0,B)", EvalError::DivisionByZero),
    ] {
        let resolver = Resolver::new();
        let sql =
            format!("SELECT {expression} AS V FROM T WHERE Id=1 UNION SELECT A FROM T WHERE Id=1");
        let result = execute(&sql, &source, &resolver, Default::default());
        assert_eq!(result, Err(QueryError::Evaluation(error)), "{sql}");
        assert!(resolver.hits.borrow().is_empty());
    }

    let mut source = Fixture::new();
    source
        .columns
        .push(Column::new("N".into(), ScalarType::Double, false).unwrap());
    source.policies.push(None);
    for row in &mut source.rows {
        row.push(Value::Double(Double::from_float(f64::INFINITY)));
    }
    let sql = "SELECT N AS V FROM T WHERE Id=1 UNION SELECT A FROM T WHERE Id=1";
    let resolver = Resolver::new();
    assert_eq!(
        execute(sql, &source, &resolver, options(false)),
        Err(QueryError::Evaluation(EvalError::NonFinite))
    );
    assert!(resolver.hits.borrow().is_empty());
}

#[test]
fn new_policy_authority_does_not_refill_work_or_allocation_and_cancels_at_every_checkpoint() {
    let source = Fixture::new();
    let sql =
        "SELECT A AS V FROM T WHERE Id<=2 UNION SELECT IIf(True,2,B) FROM T WHERE Id=1 ORDER BY V";
    let resolver = Resolver::new();
    let observed = Cell::new(0);
    let observe = || {
        observed.set(observed.get() + 1);
        false
    };
    let expected = execute(
        sql,
        &source,
        &resolver,
        ExecutionOptions {
            cancelled: Some(&observe),
            ..options(false)
        },
    )
    .unwrap();
    assert_eq!(expected, texts(&["2", "a"]));
    for stop in 1..=observed.get() {
        let calls = Cell::new(0);
        let cancel = || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        };
        let result = execute(
            sql,
            &source,
            &Resolver::new(),
            ExecutionOptions {
                cancelled: Some(&cancel),
                ..options(false)
            },
        );
        assert!(
            matches!(
                result,
                Err(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
            ),
            "checkpoint {stop}"
        );
    }
    for limits in [
        QueryLimits {
            bytes: 1,
            ..Default::default()
        },
        QueryLimits {
            steps: 1,
            ..Default::default()
        },
    ] {
        let result = execute(
            sql,
            &source,
            &Resolver::new(),
            ExecutionOptions {
                limits,
                ..options(false)
            },
        );
        assert!(matches!(result, Err(QueryError::Limit(_))));
    }
}

use super::support::{Numbers, PERMUTATIONS, options, run};
use std::cell::Cell;
use tablecraft_core::values::{Decimal, Double, Value};
use tablecraft_expressions::{
    EvalError,
    comparison::{Comparison, compare},
};
use tablecraft_query::{ExecutionOptions, QueryError, QueryLimits, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

#[test]
fn decimal_currency_binary_partitions_are_stable_in_every_encounter_order() {
    for order in PERMUTATIONS {
        let source = Numbers::mixed(order, "CDec('0.1')", "CDbl(0.1)", "CCur('0.1')");
        for declared in [false, true] {
            let grouped = run(
                "SELECT K,Count(*) AS N FROM Numbers GROUP BY K ORDER BY K",
                &source,
                declared,
            )
            .unwrap();
            assert_eq!(grouped.rows().len(), 2);
            assert_eq!(grouped.rows()[0][1], Value::Int64(2));
            assert_eq!(
                grouped.rows()[1],
                vec![Value::Double(Double::from_float(0.1)), Value::Int64(1)]
            );
            assert!(matches!(
                grouped.rows()[0][0],
                Value::Decimal(_) | Value::Currency(_)
            ));

            for sql in [
                "SELECT DISTINCT K FROM Numbers ORDER BY K",
                "SELECT K FROM Numbers UNION SELECT K FROM Numbers ORDER BY K",
            ] {
                let result = run(sql, &source, declared).unwrap();
                assert_eq!(result.rows().len(), 2, "{order:?}: {sql}");
                assert!(matches!(
                    result.rows()[0][0],
                    Value::Decimal(_) | Value::Currency(_)
                ));
                assert_eq!(result.rows()[1][0], Value::Double(Double::from_float(0.1)));
            }

            let top = run("SELECT TOP 1 K FROM Numbers ORDER BY K", &source, declared).unwrap();
            assert_eq!(top.rows().len(), 2);
            for sql in [
                "SELECT Min(K),Max(K) FROM Numbers",
                "SELECT DMin('K','Numbers'),DMax('K','Numbers')",
            ] {
                let extremes = run(sql, &source, declared).unwrap();
                assert!(matches!(
                    extremes.rows()[0][0],
                    Value::Decimal(_) | Value::Currency(_)
                ));
                assert_eq!(
                    extremes.rows()[0][1],
                    Value::Double(Double::from_float(0.1))
                );
            }
        }
    }
}

#[test]
fn exact_integral_binary_keys_retain_values_beyond_binary64_integer_precision() {
    for order in PERMUTATIONS {
        let source = Numbers::mixed(
            order,
            "CLngLng('9007199254740993')",
            "CDbl(9007199254740992)",
            "CDec('9007199254740992')",
        );
        for declared in [false, true] {
            let result = run(
                "SELECT K,Count(*) FROM Numbers GROUP BY K ORDER BY K",
                &source,
                declared,
            )
            .unwrap();
            assert_eq!(result.rows().len(), 2);
            assert_eq!(result.rows()[0][1], Value::Int64(2));
            assert_eq!(
                result.rows()[1],
                vec![Value::Int64(9_007_199_254_740_993), Value::Int64(1)]
            );
            let top = run(
                "SELECT TOP 1 K FROM Numbers ORDER BY K DESC",
                &source,
                declared,
            )
            .unwrap();
            assert_eq!(top.rows(), &[vec![Value::Int64(9_007_199_254_740_993)]]);
        }
    }
}

#[test]
fn admitted_numeric_key_equalities_do_not_change_scalar_comparison_coercion() {
    let source = MemorySource::default();
    for (left, right) in [("CDec(1)", "CDbl(1)"), ("CCur(1)", "CSng(1)")] {
        let sql = format!(
            "SELECT DISTINCT IIf(Id=1,{left},{right}) AS K FROM (SELECT 1 AS Id UNION ALL SELECT 2) X"
        );
        assert_eq!(run(&sql, &source, false).unwrap().rows().len(), 1);
    }
    assert_eq!(
        compare(
            Comparison::Equal,
            &Value::Decimal(Decimal::new(1, 0).unwrap()),
            &Value::Double(Double::from_float(1.0)),
            None
        ),
        Err(EvalError::UnsupportedNumericCombination),
    );
    assert_eq!(
        compare(
            Comparison::Equal,
            &Value::Int64(9_007_199_254_740_993),
            &Value::Double(Double::from_float(1.0)),
            None
        ),
        Err(EvalError::PrecisionLoss),
    );
}

#[test]
fn mixed_key_comparison_keeps_eager_failures_budgets_and_all_cancellation_checks() {
    let source = Numbers::mixed([1, 2, 3], "CDec('0.1')", "CDbl(0.1)", "CCur('0.1')");
    assert!(
        run(
            "SELECT DISTINCT IIf(Id=3,1/0,K) FROM Numbers",
            &source,
            false
        )
        .is_err()
    );
    let syntax = parse_query("SELECT K FROM Numbers ORDER BY K", Default::default()).unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    let visits = Cell::new(0);
    let count = || {
        visits.set(visits.get() + 1);
        false
    };
    plan.execute(
        &source,
        &[],
        ExecutionOptions {
            cancelled: Some(&count),
            ..options(false)
        },
    )
    .unwrap();
    let total = visits.get();
    for boundary in 1..=total {
        visits.set(0);
        let cancelled = || {
            visits.set(visits.get() + 1);
            visits.get() == boundary
        };
        assert!(
            matches!(
                plan.execute(
                    &source,
                    &[],
                    ExecutionOptions {
                        cancelled: Some(&cancelled),
                        ..options(false)
                    }
                ),
                Err(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
            ),
            "checkpoint {boundary}"
        );
    }
    assert!(matches!(
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                limits: QueryLimits {
                    steps: 1,
                    ..Default::default()
                },
                ..options(false)
            }
        ),
        Err(QueryError::Limit(_))
    ));
}

#[test]
fn fixed_crosstab_headings_use_exact_mixed_numeric_membership() {
    for order in PERMUTATIONS {
        let source = Numbers::mixed(
            order,
            "CLngLng('9007199254740993')",
            "CDbl(9007199254740992)",
            "CDec('9007199254740992')",
        );
        for declared in [false, true] {
            let result = run(
                "TRANSFORM Count(*) SELECT 7 AS G FROM Numbers GROUP BY 7 \
                 PIVOT K IN (9007199254740992,9007199254740993)",
                &source,
                declared,
            )
            .unwrap();
            assert_eq!(
                result.columns(),
                ["G", "9007199254740992", "9007199254740993"]
            );
            assert_eq!(
                result.rows(),
                &[vec![Value::Int64(7), Value::Int64(2), Value::Int64(1)]]
            );
        }
    }
}

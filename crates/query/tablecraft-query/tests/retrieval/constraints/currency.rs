use super::*;
use tablecraft_core::values::Currency;

impl Restricted {
    fn money(ignore: bool) -> Self {
        Self {
            source: MemorySource::new(vec![table(
                "T",
                &[("X", ScalarType::Currency), ("Y", ScalarType::Int32)],
                [
                    None,
                    Some(i64::MIN),
                    Some(-1),
                    Some(0),
                    Some(1),
                    Some(9_007_199_254_740_993),
                    Some(i64::MAX),
                ]
                .into_iter()
                .map(|value| {
                    vec![
                        value.map_or(Value::Null, |value| {
                            Value::Currency(Currency::from_scaled(value))
                        }),
                        Value::Int32(1),
                    ]
                })
                .collect(),
            )])
            .unwrap(),
            seen: Cell::new(None),
            scans: Cell::new(0),
            ignore,
        }
    }
}

#[test]
fn currency_hints_keep_exact_literals_reversed_bounds_nulls_and_residuals() {
    for (filter, expected) in [
        (
            "X>=CCur(-0.0001) AND X<CCur(0.0001)",
            ScanConstraint::Currency {
                column: 0,
                lower: Included(Currency::from_scaled(-1)),
                upper: Excluded(Currency::from_scaled(1)),
            },
        ),
        (
            "CCur(0.0001)<X",
            ScanConstraint::Currency {
                column: 0,
                lower: Excluded(Currency::from_scaled(1)),
                upper: Unbounded,
            },
        ),
        (
            "X=CCur(0)",
            ScanConstraint::Currency {
                column: 0,
                lower: Included(Currency::from_scaled(0)),
                upper: Included(Currency::from_scaled(0)),
            },
        ),
        (
            "X=CCur(0.00005)",
            ScanConstraint::Currency {
                column: 0,
                lower: Included(Currency::from_scaled(1)),
                upper: Included(Currency::from_scaled(1)),
            },
        ),
        ("X IS NULL", ScanConstraint::Null { column: 0 }),
        (
            "X IS NOT NULL",
            ScanConstraint::Currency {
                column: 0,
                lower: Unbounded,
                upper: Unbounded,
            },
        ),
        (
            "X IS NULL AND X>CCur(0)",
            ScanConstraint::Currency {
                column: 0,
                lower: Included(Currency::from_scaled(0)),
                upper: Excluded(Currency::from_scaled(0)),
            },
        ),
    ] {
        let source = Restricted::money(false);
        let ignoring = Restricted::money(true);
        let sql = format!("SELECT X FROM T WHERE {filter}");
        assert_eq!(
            run(&sql, &source).unwrap().rows(),
            run(&sql, &ignoring).unwrap().rows(),
            "{filter}"
        );
        assert_eq!(source.seen.get(), Some(expected));
    }
}

#[test]
fn currency_parameters_preserve_full_scaled_precision_across_reused_plans() {
    let source = Restricted::money(false);
    let query = tablecraft_sql::parse_query(
        "PARAMETERS P CURRENCY; SELECT X FROM T WHERE X=P",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    assert_eq!(source.scans.get(), 0);
    for scaled in [i64::MIN, 9_007_199_254_740_993, i64::MAX] {
        let value = Currency::from_scaled(scaled);
        let result = plan
            .execute(
                &source,
                &[("P", Value::Currency(value))],
                Default::default(),
            )
            .unwrap();
        assert_eq!(result.rows(), [vec![Value::Currency(value)]]);
        assert_eq!(
            source.seen.get(),
            Some(ScanConstraint::Currency {
                column: 0,
                lower: Included(value),
                upper: Included(value),
            })
        );
    }
    source.seen.set(None);
    let result = plan
        .execute(&source, &[("P", Value::Null)], Default::default())
        .unwrap();
    assert!(result.rows().is_empty());
    assert_eq!(source.seen.get(), None);
}

#[test]
fn unsafe_or_invalid_currency_constants_decline_hints_without_hiding_eager_errors() {
    for filter in [
        "X=0",
        "X=CCur('0')",
        "X=CCur(1/0)",
        "X=CCur(1e300)",
        "X>=CCur(0) AND 1/0=0",
        "X>=CCur(0) AND Y=1",
        "X=CCur(0) OR X IS NULL",
    ] {
        let source = Restricted::money(false);
        let ignoring = Restricted::money(true);
        let sql = format!("SELECT X FROM T WHERE {filter}");
        assert_eq!(
            run(&sql, &source).map(|r| r.rows().to_vec()),
            run(&sql, &ignoring).map(|r| r.rows().to_vec()),
            "{filter}"
        );
        assert_eq!(source.seen.get(), None, "{filter}");
    }
    let source =
        MemorySource::new(vec![table("T", &[("X", ScalarType::Currency)], vec![])]).unwrap();
    for filter in ["X=CCur(1e300)", "X=CCur(1/0)"] {
        assert!(
            run(&format!("SELECT X FROM T WHERE {filter}"), &source)
                .unwrap()
                .rows()
                .is_empty()
        );
    }
}

#[test]
fn currency_hint_work_shares_every_observed_cancellation_checkpoint() {
    let source = Restricted::money(false);
    let query = tablecraft_sql::parse_query(
        "SELECT X FROM T WHERE X>=CCur(-0.0001) AND X<=CCur(0.0001)",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    let observed = Cell::new(0);
    let count = || {
        observed.set(observed.get() + 1);
        false
    };
    plan.execute(
        &source,
        &[],
        ExecutionOptions {
            cancelled: Some(&count),
            ..Default::default()
        },
    )
    .unwrap();
    for stop in 1..=observed.get() {
        let checks = Cell::new(0);
        let cancelled = || {
            checks.set(checks.get() + 1);
            checks.get() >= stop
        };
        assert!(matches!(
            plan.execute(
                &source,
                &[],
                ExecutionOptions {
                    cancelled: Some(&cancelled),
                    ..Default::default()
                }
            ),
            Err(QueryError::Cancelled
                | QueryError::Evaluation(tablecraft_expressions::EvalError::Cancelled))
        ));
    }
}

#[test]
fn currency_integer_overflow_cannot_be_hidden_by_partial_pushdown() {
    for filter in [
        "X=922337203685478",
        "X=922337203685478 AND Y<0",
        "Y<0 AND X=922337203685478",
    ] {
        for ignore in [false, true] {
            let source = Restricted::money(ignore);
            assert_eq!(
                run(&format!("SELECT X FROM T WHERE {filter}"), &source)
                    .map(|result| result.rows().to_vec()),
                Err(tablecraft_expressions::EvalError::Overflow.into()),
                "{filter}"
            );
            assert_eq!(source.seen.get(), None);
        }

        let empty = MemorySource::new(vec![table(
            "T",
            &[("X", ScalarType::Currency), ("Y", ScalarType::Int32)],
            vec![],
        )])
        .unwrap();
        assert!(
            run(&format!("SELECT X FROM T WHERE {filter}"), &empty)
                .unwrap()
                .rows()
                .is_empty()
        );
    }
}

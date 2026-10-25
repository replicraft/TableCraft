use super::*;
use std::ops::Bound;
use tablecraft_core::values::LegacyDateTime;

fn date(serial: f64) -> LegacyDateTime {
    LegacyDateTime::from_ole_days(serial).unwrap()
}

pub(super) fn contains(
    value: LegacyDateTime,
    lower: Bound<LegacyDateTime>,
    upper: Bound<LegacyDateTime>,
) -> bool {
    let after = match lower {
        Unbounded => true,
        Included(bound) => !value.temporal_cmp(bound).is_lt(),
        Excluded(bound) => value.temporal_cmp(bound).is_gt(),
    };
    let before = match upper {
        Unbounded => true,
        Included(bound) => !value.temporal_cmp(bound).is_gt(),
        Excluded(bound) => value.temporal_cmp(bound).is_lt(),
    };
    after && before
}

impl Restricted {
    fn dates(ignore: bool) -> Self {
        let values = [
            None,
            Some(-1.25),
            Some(-1.75),
            Some(-0.25),
            Some(0.25),
            Some(-0.0),
            Some(0.0),
            Some(0.5),
            Some(1.25),
        ];
        Self {
            source: MemorySource::new(vec![table(
                "T",
                &[("X", ScalarType::DateTime), ("Y", ScalarType::Int32)],
                values
                    .into_iter()
                    .map(|value| {
                        vec![
                            value.map_or(Value::Null, |value| Value::DateTime(date(value))),
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
fn date_hints_keep_captured_literals_temporal_intersections_nulls_and_residuals() {
    for (filter, expected) in [
        (
            "X>=#12/29/1899 06:00# AND X<#12/29/1899 18:00#",
            ScanConstraint::DateTime {
                column: 0,
                lower: Included(date(-1.25)),
                upper: Excluded(date(-1.75)),
            },
        ),
        (
            "#12/29/1899 06:00#<=X",
            ScanConstraint::DateTime {
                column: 0,
                lower: Included(date(-1.25)),
                upper: Unbounded,
            },
        ),
        (
            "X=#12/30/1899 06:00#",
            ScanConstraint::DateTime {
                column: 0,
                lower: Included(date(0.25)),
                upper: Included(date(0.25)),
            },
        ),
        ("X IS NULL", ScanConstraint::Null { column: 0 }),
        (
            "X IS NOT NULL",
            ScanConstraint::DateTime {
                column: 0,
                lower: Unbounded,
                upper: Unbounded,
            },
        ),
        (
            "X IS NULL AND X>#12/30/1899#",
            ScanConstraint::DateTime {
                column: 0,
                lower: Included(date(0.0)),
                upper: Excluded(date(0.0)),
            },
        ),
    ] {
        let selected = Restricted::dates(false);
        let superset = Restricted::dates(true);
        let sql = format!("SELECT X FROM T WHERE {filter}");
        assert_eq!(
            run(&sql, &selected).unwrap().rows(),
            run(&sql, &superset).unwrap().rows()
        );
        assert_eq!(selected.seen.get(), Some(expected));
    }
}

#[test]
fn typed_date_parameters_reuse_plans_without_canonicalizing_aliases() {
    let source = Restricted::dates(false);
    let syntax = parse_query(
        "PARAMETERS P DATETIME; SELECT X FROM T WHERE P=X",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    assert_eq!(source.scans.get(), 0);
    for serial in [-1.25, -1.75, -0.25, 0.25, -0.0, 0.0, 0.5] {
        let value = date(serial);
        let rows = plan
            .execute(
                &source,
                &[("P", Value::DateTime(value))],
                Default::default(),
            )
            .unwrap();
        assert!(rows.rows().iter().all(
            |row| matches!(&row[0], Value::DateTime(actual) if actual.temporal_cmp(value).is_eq())
        ));
        assert_eq!(
            source.seen.get(),
            Some(ScanConstraint::DateTime {
                column: 0,
                lower: Included(value),
                upper: Included(value)
            })
        );
        if serial.abs() == 0.25 {
            assert_eq!(
                rows.rows(),
                &[
                    vec![Value::DateTime(date(-0.25))],
                    vec![Value::DateTime(date(0.25))]
                ]
            );
        }
    }

    let syntax = parse_query(
        "PARAMETERS P DATETIME,Q DATETIME; SELECT X FROM T WHERE X>=P AND X<Q",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    let result = plan
        .execute(
            &source,
            &[
                ("P", Value::DateTime(date(-0.25))),
                ("Q", Value::DateTime(date(0.25))),
            ],
            Default::default(),
        )
        .unwrap();
    assert!(result.rows().is_empty());
    assert_eq!(
        source.seen.get(),
        Some(ScanConstraint::DateTime {
            column: 0,
            lower: Included(date(-0.25)),
            upper: Excluded(date(0.25)),
        }),
    );
}

#[test]
fn coercive_numeric_and_conversion_call_operands_decline_date_hints() {
    for sql in [
        "SELECT X FROM T WHERE X=0.25",
        "SELECT X FROM T WHERE X=CDate(0.25)",
        "SELECT X FROM T WHERE X=CDec(0.25)",
    ] {
        let source = Restricted::dates(false);
        let result = run(sql, &source).unwrap();
        assert_eq!(source.seen.get(), None);
        let expected = if sql.contains("CDec") { 1 } else { 2 };
        assert_eq!(result.rows().len(), expected);
    }
}

#[test]
fn partly_unsafe_and_or_and_computed_fields_never_extract_a_date_hint() {
    for filter in [
        "X=#12/30/1899 06:00# AND 1/0=0",
        "X=#12/30/1899 06:00# OR 1/0=0",
        "X>=#12/30/1899# AND Y=1",
        "CDate(X)=#12/30/1899 06:00#",
    ] {
        let source = Restricted::dates(false);
        let result = run(&format!("SELECT X FROM T WHERE {filter}"), &source);
        if filter.contains("1/0") {
            assert!(matches!(
                result,
                Err(QueryError::Evaluation(EvalError::DivisionByZero))
            ));
        } else if filter.contains("CDate(X)") {
            assert!(matches!(
                result,
                Err(QueryError::Evaluation(EvalError::InvalidNullConversion))
            ));
        } else {
            result.unwrap();
        }
        assert_eq!(source.seen.get(), None);
    }
}

#[test]
fn date_hint_analysis_obeys_each_observed_cancellation_checkpoint() {
    let source = Restricted::dates(false);
    let syntax = parse_query(
        "SELECT X FROM T WHERE X>=#12/29/1899 06:00# AND X<=#12/30/1899 18:00#",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
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
            Err(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
        ));
    }
}

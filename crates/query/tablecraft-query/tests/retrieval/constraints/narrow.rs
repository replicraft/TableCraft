use super::*;

impl Restricted {
    fn narrow(byte: bool, ignore: bool) -> Self {
        let values = if byte {
            [None, Some(0), Some(1), Some(127), Some(128), Some(255)]
        } else {
            [None, Some(-32768), Some(-1), Some(0), Some(1), Some(32767)]
        };
        let kind = if byte {
            ScalarType::Byte
        } else {
            ScalarType::Int16
        };
        let rows = values
            .into_iter()
            .map(|value| {
                let value = value.map_or(Value::Null, |value| {
                    if byte {
                        Value::Byte(value as u8)
                    } else {
                        Value::Int16(value as i16)
                    }
                });
                vec![value, Value::Int32(1)]
            })
            .collect();
        Self {
            source: MemorySource::new(vec![table(
                "T",
                &[("X", kind), ("Y", ScalarType::Int32)],
                rows,
            )])
            .unwrap(),
            seen: Cell::new(None),
            scans: Cell::new(0),
            ignore,
        }
    }
}

#[test]
fn narrow_hints_keep_width_casts_signed_extremes_and_original_filters() {
    for (byte, filter, expected) in [
        (
            true,
            "X>=0 AND X<128",
            ScanConstraint::Byte {
                column: 0,
                lower: Included(0),
                upper: Excluded(128),
            },
        ),
        (
            true,
            "255=X",
            ScanConstraint::Byte {
                column: 0,
                lower: Included(255),
                upper: Included(255),
            },
        ),
        (
            true,
            "X=CByte(1)",
            ScanConstraint::Byte {
                column: 0,
                lower: Included(1),
                upper: Included(1),
            },
        ),
        (
            false,
            "X>=-32768 AND X<0",
            ScanConstraint::Int16 {
                column: 0,
                lower: Included(-32768),
                upper: Excluded(0),
            },
        ),
        (
            false,
            "32767=X",
            ScanConstraint::Int16 {
                column: 0,
                lower: Included(32767),
                upper: Included(32767),
            },
        ),
        (
            false,
            "X=CInt(-1)",
            ScanConstraint::Int16 {
                column: 0,
                lower: Included(-1),
                upper: Included(-1),
            },
        ),
        (true, "X IS NULL", ScanConstraint::Null { column: 0 }),
        (
            false,
            "X IS NOT NULL",
            ScanConstraint::Int16 {
                column: 0,
                lower: Unbounded,
                upper: Unbounded,
            },
        ),
    ] {
        let source = Restricted::narrow(byte, false);
        let ignoring = Restricted::narrow(byte, true);
        let sql = format!("SELECT X FROM T WHERE {filter}");
        assert_eq!(
            run(&sql, &source).unwrap().rows(),
            run(&sql, &ignoring).unwrap().rows(),
            "{sql}"
        );
        assert_eq!(source.seen.get(), Some(expected));
    }
}

#[test]
fn narrow_parameters_rebind_exactly_without_null_equality_becoming_null_selection() {
    for byte in [false, true] {
        let source = Restricted::narrow(byte, false);
        let kind = if byte { "BYTE" } else { "SHORT" };
        let query = tablecraft_sql::parse_query(
            &format!("PARAMETERS P {kind}; SELECT X FROM T WHERE X=P"),
            Default::default(),
        )
        .unwrap();
        let plan = prepare(query.query(), &source).unwrap();
        assert_eq!(source.scans.get(), 0);
        let values = if byte {
            vec![Value::Byte(0), Value::Byte(255)]
        } else {
            vec![Value::Int16(-32768), Value::Int16(32767)]
        };
        for value in values {
            let result = plan
                .execute(&source, &[("P", value.clone())], Default::default())
                .unwrap();
            assert_eq!(result.rows(), [vec![value]]);
        }
        source.seen.set(None);
        assert!(
            plan.execute(&source, &[("P", Value::Null)], Default::default())
                .unwrap()
                .rows()
                .is_empty()
        );
        assert_eq!(source.seen.get(), None);
    }
}

#[test]
fn out_of_width_or_unsafe_narrow_bounds_decline_without_hiding_eager_errors() {
    for (byte, filter) in [
        (true, "X<256"),
        (false, "X>-32769"),
        (true, "X=CByte(256)"),
        (false, "X=CInt(32768)"),
        (true, "X>=1 AND 1/0=0"),
        (false, "X>=1 AND Y=1"),
        (true, "X=1 OR X IS NULL"),
        (false, "X=CInt(1/0)"),
    ] {
        let source = Restricted::narrow(byte, false);
        let ignoring = Restricted::narrow(byte, true);
        let sql = format!("SELECT X FROM T WHERE {filter}");
        assert_eq!(
            run(&sql, &source).map(|r| r.rows().to_vec()),
            run(&sql, &ignoring).map(|r| r.rows().to_vec()),
            "{filter}"
        );
        assert_eq!(source.seen.get(), None, "{filter}");
    }
    for (kind, filter) in [
        (ScalarType::Byte, "X=CByte(256)"),
        (ScalarType::Int16, "X=CInt(32768)"),
    ] {
        let source = MemorySource::new(vec![table("T", &[("X", kind)], vec![])]).unwrap();
        assert!(
            run(&format!("SELECT X FROM T WHERE {filter}"), &source)
                .unwrap()
                .rows()
                .is_empty()
        );
    }
}

#[test]
fn narrow_hint_extraction_and_residuals_share_every_observed_cancellation_checkpoint() {
    for byte in [false, true] {
        let source = Restricted::narrow(byte, false);
        let query = tablecraft_sql::parse_query(
            "SELECT X FROM T WHERE X>=CInt(0) AND X<=CByte(1)",
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
}

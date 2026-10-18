use super::*;
use std::ops::Bound::{Excluded, Included, Unbounded};
use tablecraft_query::source::ScanConstraint;

struct Restricted {
    source: MemorySource,
    seen: Cell<Option<ScanConstraint>>,
    scans: Cell<usize>,
    ignore: bool,
}
impl Restricted {
    fn new(ignore: bool) -> Self {
        Self {
            source: MemorySource::new(vec![table(
                "T",
                &[("X", ScalarType::Int32), ("Y", ScalarType::Int32)],
                [
                    None,
                    Some(i32::MIN),
                    Some(-2),
                    Some(0),
                    Some(2),
                    Some(4),
                    Some(i32::MAX),
                ]
                .into_iter()
                .map(|v| vec![v.map_or(Value::Null, Value::Int32), Value::Int32(1)])
                .collect(),
            )])
            .unwrap(),
            seen: Cell::new(None),
            scans: Cell::new(0),
            ignore,
        }
    }
}
impl Source for Restricted {
    fn columns(&self, table: &str) -> Result<&[Column], QueryError> {
        self.source.columns(table)
    }
    fn scan(
        &self,
        table: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.scans.set(self.scans.get() + 1);
        self.source.scan(table, visitor)
    }
    fn scan_constrained(
        &self,
        table: &str,
        constraint: ScanConstraint,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.seen.set(Some(constraint));
        self.source.scan(table, &mut |row| {
            let matches = match (constraint, &row[constraint.column()]) {
                (ScanConstraint::Null { .. }, Value::Null) => true,
                (ScanConstraint::Int32 { lower, upper, .. }, Value::Int32(value)) => {
                    use std::ops::RangeBounds;
                    (lower, upper).contains(value)
                }
                _ => false,
            };
            if self.ignore || matches {
                visitor(row)?;
            }
            Ok(())
        })
    }
}

#[test]
fn whole_safe_filters_preserve_residuals_bounds_nulls_and_source_order() {
    let cases = [
        (
            "X = 2",
            ScanConstraint::Int32 {
                column: 0,
                lower: Included(2),
                upper: Included(2),
            },
        ),
        (
            "1 < X AND X <= 4",
            ScanConstraint::Int32 {
                column: 0,
                lower: Excluded(1),
                upper: Included(4),
            },
        ),
        (
            "X >= -2 AND X > -2 AND X < 4",
            ScanConstraint::Int32 {
                column: 0,
                lower: Excluded(-2),
                upper: Excluded(4),
            },
        ),
        (
            "X <= 4 AND X < 4",
            ScanConstraint::Int32 {
                column: 0,
                lower: Unbounded,
                upper: Excluded(4),
            },
        ),
        ("X IS NULL", ScanConstraint::Null { column: 0 }),
        (
            "X IS NOT NULL",
            ScanConstraint::Int32 {
                column: 0,
                lower: Unbounded,
                upper: Unbounded,
            },
        ),
        (
            "X >= -2147483648",
            ScanConstraint::Int32 {
                column: 0,
                lower: Included(i32::MIN),
                upper: Unbounded,
            },
        ),
        (
            "X IS NULL AND X = 0",
            ScanConstraint::Int32 {
                column: 0,
                lower: Included(0),
                upper: Excluded(0),
            },
        ),
    ];
    for (filter, expected) in cases {
        let restricted = Restricted::new(false);
        let ignoring = Restricted::new(true);
        let sql = format!("SELECT t.X FROM T AS t WHERE {filter}");
        let actual = run(&sql, &restricted).unwrap();
        assert_eq!(
            actual.rows(),
            run(&sql, &ignoring).unwrap().rows(),
            "{filter}"
        );
        assert_eq!(restricted.seen.get(), Some(expected), "{filter}");
        assert_eq!(restricted.scans.get(), 0);
    }
}

#[test]
fn unsafe_or_unqualified_predicates_keep_full_scans_and_eager_errors() {
    for filter in [
        "X = 2 OR X = 4",
        "X > 0 AND Y = 1",
        "X BETWEEN 4 AND 0",
        "X + 1 > 0",
        "X > 2147483648",
        "X > 0 AND 1 / 0 = 0",
        "X = 'secret'",
        "X = NULL",
    ] {
        let source = Restricted::new(false);
        let actual = run(&format!("SELECT X FROM T WHERE {filter}"), &source);
        assert_eq!(source.seen.get(), None, "{filter}");
        assert_eq!(source.scans.get(), 1, "{filter}");
        if filter.contains("1 / 0") {
            assert!(actual.is_err(), "eager error was hidden");
        }
    }
    let source = Restricted::new(false);
    run("SELECT a.X FROM T AS a, T AS b WHERE a.X = 2", &source).unwrap();
    assert_eq!(source.seen.get(), None);
    assert_eq!(source.scans.get(), 2);
}

#[test]
fn parameters_are_rebound_without_turning_null_or_overflow_into_bounds() {
    let source = Restricted::new(false);
    let parsed = parse_query(
        "PARAMETERS p LONG; SELECT X FROM T WHERE X >= p",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    for value in [Value::Int32(-2), Value::Int32(4), Value::Null] {
        source.seen.set(None);
        let params = [("p", value.clone())];
        let actual = plan
            .execute(&source, &params, ExecutionOptions::default())
            .unwrap();
        let ignoring = Restricted::new(true);
        assert_eq!(
            actual.rows(),
            plan.execute(&ignoring, &params, ExecutionOptions::default())
                .unwrap()
                .rows()
        );
        assert_eq!(source.seen.get().is_some(), value != Value::Null);
    }
    source.seen.set(None);
    let parsed = parse_query(
        "PARAMETERS p BIGINT; SELECT X FROM T WHERE X = -(-p)",
        ParseLimits::default(),
    )
    .unwrap();
    let result = prepare(parsed.query(), &source).unwrap().execute(
        &source,
        &[("p", Value::Int64(i64::MIN))],
        ExecutionOptions::default(),
    );
    assert!(result.is_err());
    assert_eq!(source.seen.get(), None);
}

#[test]
fn constrained_sources_cannot_hide_rejected_rows_or_late_source_failures() {
    struct Broken {
        columns: Vec<Column>,
        invalid: bool,
    }
    impl Source for Broken {
        fn columns(&self, _: &str) -> Result<&[Column], QueryError> {
            Ok(&self.columns)
        }
        fn scan(
            &self,
            _: &str,
            _: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
        ) -> Result<(), QueryError> {
            panic!("expected constrained scan")
        }
        fn scan_constrained(
            &self,
            _: &str,
            _: ScanConstraint,
            visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
        ) -> Result<(), QueryError> {
            if self.invalid {
                let _ = visitor(&[Value::Text("private payload".into())]);
                let _ = visitor(&[Value::Int32(1)]);
                Ok(())
            } else {
                visitor(&[Value::Int32(1)])?;
                Err(QueryError::Source)
            }
        }
    }
    for invalid in [false, true] {
        let source = Broken {
            columns: vec![Column::new("X".into(), ScalarType::Int32, true).unwrap()],
            invalid,
        };
        let error = run("SELECT X FROM T WHERE X = 1", &source).err().unwrap();
        if invalid {
            assert!(matches!(error, QueryError::Schema(_)));
        } else {
            assert_eq!(error, QueryError::Source);
        }
    }
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config {cases:256,rng_seed:proptest::test_runner::RngSeed::Fixed(0x5441_424c_5345_454b),..Default::default()})]
    #[test]
    fn constrained_ranges_match_independent_numeric_filtering(a in proptest::prelude::any::<i32>(), b in proptest::prelude::any::<i32>(), open_low in proptest::prelude::any::<bool>(), open_high in proptest::prelude::any::<bool>()) {
        let source = Restricted::new(false);
        let sql = format!("SELECT X FROM T WHERE {a} {} X AND X {} {b}", if open_low { "<" } else { "<=" }, if open_high { "<" } else { "<=" });
        let result = run(&sql, &source).unwrap();
        let expected: Vec<_> = [i32::MIN, -2, 0, 2, 4, i32::MAX].into_iter()
            .filter(|v| if open_low { *v > a } else { *v >= a })
            .filter(|v| if open_high { *v < b } else { *v <= b })
            .map(|v| vec![Value::Int32(v)]).collect();
        proptest::prop_assert_eq!(result.rows(), expected);
        proptest::prop_assert!(source.seen.get().is_some());
    }
}

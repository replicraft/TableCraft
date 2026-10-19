use super::*;

impl Restricted {
    fn big(ignore: bool) -> Self {
        Self {
            source: MemorySource::new(vec![table(
                "T",
                &[("X", ScalarType::Int64), ("Y", ScalarType::Int32)],
                [
                    None,
                    Some(i64::MIN),
                    Some(-9_007_199_254_740_993),
                    Some(-1),
                    Some(0),
                    Some(1),
                    Some(9_007_199_254_740_992),
                    Some(9_007_199_254_740_993),
                    Some(i64::MAX),
                ]
                .into_iter()
                .map(|value| vec![value.map_or(Value::Null, Value::Int64), Value::Int32(1)])
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
fn exact_int64_filters_keep_bounds_residuals_nulls_and_eager_failures() {
    for (filter, expected) in [
        (
            "X=9007199254740993",
            ScanConstraint::Int64 {
                column: 0,
                lower: Included(9_007_199_254_740_993),
                upper: Included(9_007_199_254_740_993),
            },
        ),
        (
            "9007199254740992<X AND X<=9223372036854775807",
            ScanConstraint::Int64 {
                column: 0,
                lower: Excluded(9_007_199_254_740_992),
                upper: Included(i64::MAX),
            },
        ),
        (
            "X>=-9223372036854775808",
            ScanConstraint::Int64 {
                column: 0,
                lower: Included(i64::MIN),
                upper: Unbounded,
            },
        ),
        ("X IS NULL", ScanConstraint::Null { column: 0 }),
        (
            "X IS NOT NULL",
            ScanConstraint::Int64 {
                column: 0,
                lower: Unbounded,
                upper: Unbounded,
            },
        ),
        (
            "X IS NULL AND X=0",
            ScanConstraint::Int64 {
                column: 0,
                lower: Included(0),
                upper: Excluded(0),
            },
        ),
    ] {
        let source = Restricted::big(false);
        let ignoring = Restricted::big(true);
        let sql = format!("SELECT X FROM T WHERE {filter}");
        assert_eq!(
            run(&sql, &source).unwrap().rows(),
            run(&sql, &ignoring).unwrap().rows(),
            "{filter}"
        );
        assert_eq!(source.seen.get(), Some(expected));
        assert_eq!(source.scans.get(), 0);
    }
    for filter in [
        "X>0 AND 1/0=0",
        "X=9007199254740992.0",
        "X>0 AND Y=1",
        "X+1>0",
        "X BETWEEN 1 AND 3",
        "X=NULL",
        "X=1 OR X=0",
    ] {
        let source = Restricted::big(false);
        let ignoring = Restricted::big(true);
        let sql = format!("SELECT X FROM T WHERE {filter}");
        let actual = run(&sql, &source);
        let expected = run(&sql, &ignoring);
        match (actual, expected) {
            (Ok(a), Ok(b)) => assert_eq!(a.rows(), b.rows()),
            (Err(a), Err(b)) => assert_eq!(a, b),
            _ => panic!("hint changed result/error"),
        }
        assert_eq!(source.seen.get(), None, "{filter}");
        assert_eq!(source.scans.get(), 1);
    }
}

#[test]
fn exact_int64_parameters_rebind_and_unary_overflow_keeps_eager_evaluation() {
    let source = Restricted::big(false);
    let ignoring = Restricted::big(true);
    let parsed = parse_query(
        "PARAMETERS p BIGINT; SELECT X FROM T WHERE X>=p",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    for value in [
        Value::Int64(i64::MIN),
        Value::Int64(9_007_199_254_740_993),
        Value::Int64(i64::MAX),
        Value::Null,
    ] {
        source.seen.set(None);
        let parameters = [("p", value.clone())];
        assert_eq!(
            plan.execute(&source, &parameters, ExecutionOptions::default())
                .unwrap()
                .rows(),
            plan.execute(&ignoring, &parameters, ExecutionOptions::default())
                .unwrap()
                .rows()
        );
        assert_eq!(source.seen.get().is_some(), value != Value::Null);
    }
    source.seen.set(None);
    let parsed = parse_query(
        "PARAMETERS p BIGINT; SELECT X FROM T WHERE X=-p",
        ParseLimits::default(),
    )
    .unwrap();
    assert!(
        prepare(parsed.query(), &source)
            .unwrap()
            .execute(
                &source,
                &[("p", Value::Int64(i64::MIN))],
                ExecutionOptions::default()
            )
            .is_err()
    );
    assert_eq!(source.seen.get(), None);
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config {cases:256,rng_seed:proptest::test_runner::RngSeed::Fixed(0x49363448494e5453),..Default::default()})]
    #[test]
    fn int64_hints_match_independent_numeric_filtering(a in proptest::prelude::any::<i64>(),b in proptest::prelude::any::<i64>(),open_low in proptest::prelude::any::<bool>(),open_high in proptest::prelude::any::<bool>()) {
        let source=Restricted::big(false);let sql=format!("SELECT X FROM T WHERE {a} {} X AND X {} {b}",if open_low{"<"}else{"<="},if open_high{"<"}else{"<="});
        let result=run(&sql,&source).unwrap();
        let expected=[i64::MIN,-9_007_199_254_740_993,-1,0,1,9_007_199_254_740_992,9_007_199_254_740_993,i64::MAX].into_iter().filter(|value|if open_low{*value>a}else{*value>=a}).filter(|value|if open_high{*value<b}else{*value<=b}).map(|value|vec![Value::Int64(value)]).collect::<Vec<_>>();
        proptest::prop_assert_eq!(result.rows(),expected);proptest::prop_assert!(source.seen.get().is_some());
    }
}

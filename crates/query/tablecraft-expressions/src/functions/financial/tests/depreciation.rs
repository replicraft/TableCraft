use super::*;

#[test]
fn depreciation_methods_and_default_factor_have_distinct_schedules() {
    near(
        run(FinancialFunction::SLN, &[2400., 300., 10.]).unwrap(),
        210.,
    );
    near(
        run(FinancialFunction::SYD, &[2400., 300., 10., 1.]).unwrap(),
        4200. / 11.,
    );
    near(
        run(FinancialFunction::SYD, &[2400., 300., 10., 10.]).unwrap(),
        420. / 11.,
    );
    near(
        run(FinancialFunction::DDB, &[2400., 300., 10., 1.]).unwrap(),
        480.,
    );
    near(
        run(FinancialFunction::DDB, &[2400., 300., 10., 2., 1.5]).unwrap(),
        306.,
    );
    near(
        run(FinancialFunction::DDB, &[2400., 300., 10., 10.]).unwrap(),
        22.1225472,
    );
    for (index, expected) in [600., 300., 100., 0.].into_iter().enumerate() {
        near(
            run(
                FinancialFunction::DDB,
                &[1200., 200., 4., (index + 1) as f64],
            )
            .unwrap(),
            expected,
        );
    }
    assert_eq!(
        run(FinancialFunction::DDB, &[1200., 200., 4., 2.]),
        run(FinancialFunction::DDB, &[1200., 200., 4., 2., 2.])
    );
    for cost in [
        Value::Byte(100),
        Value::Int16(100),
        Value::Int32(100),
        Value::Int64(100),
        Value::Single(Single::from_float(100.)),
        Value::Currency(Currency::from_scaled(1000000)),
        Value::Decimal(Decimal::new(100, 0).unwrap()),
    ] {
        assert_eq!(
            FinancialFunction::SLN.apply(&[cost, Value::Int32(20), Value::Int32(4)]),
            Ok(value(20.))
        );
    }
}
#[test]
fn signed_and_short_life_runtime_profiles_are_explicit_not_prorated() {
    near(
        run(FinancialFunction::SLN, &[-100., 20., 4.]).unwrap(),
        -30.,
    );
    near(
        run(FinancialFunction::SLN, &[100., 20., -4.]).unwrap(),
        -20.,
    );
    near(
        run(FinancialFunction::SYD, &[-100., 20., 4., 1.]).unwrap(),
        -48.,
    );
    near(
        run(FinancialFunction::DDB, &[100., 20., 2., 1., 1.]).unwrap(),
        80.,
    );
    near(
        run(FinancialFunction::DDB, &[100., 20., 2., 2., 1.]).unwrap(),
        0.,
    );
    near(
        run(FinancialFunction::DDB, &[100., 20., 1.5, 1.5, 1.]).unwrap(),
        80.,
    );
    near(
        run(FinancialFunction::DDB, &[100., 20., 5., 0.5]).unwrap(),
        40.,
    );
    near(
        run(FinancialFunction::DDB, &[-100., 20., 5., 1.]).unwrap(),
        0.,
    );
    near(
        run(FinancialFunction::DDB, &[100., 200., 5., 1.]).unwrap(),
        -100.,
    );
    near(
        run(FinancialFunction::DDB, &[100., 200., 5., 2.]).unwrap(),
        0.,
    );
    assert_eq!(
        run(FinancialFunction::DDB, &[100., 20., 3., 2.5, 4.]),
        Err(EvalError::Domain)
    );
}
#[test]
fn invalid_domains_carriers_arity_and_representable_extreme_results_are_checked() {
    assert_eq!(
        run(FinancialFunction::SLN, &[100., 20., 0.]),
        Err(EvalError::DivisionByZero)
    );
    for (function, args) in [
        (FinancialFunction::SYD, vec![100., -1., 4., 1.]),
        (FinancialFunction::SYD, vec![100., 20., 4., 0.]),
        (FinancialFunction::SYD, vec![100., 20., 4., 5.]),
        (FinancialFunction::DDB, vec![100., -1., 4., 1.]),
        (FinancialFunction::DDB, vec![100., 20., 4., 0.]),
        (FinancialFunction::DDB, vec![100., 20., 4., 5.]),
        (FinancialFunction::DDB, vec![100., 20., 4., 1., 0.]),
    ] {
        assert_eq!(run(function, &args), Err(EvalError::Domain));
    }
    for (function, args) in [
        (FinancialFunction::SLN, vec![100., 20., 4.]),
        (FinancialFunction::SYD, vec![100., 20., 4., 1.]),
        (FinancialFunction::DDB, vec![100., 20., 4., 1., 2.]),
    ] {
        for index in 0..args.len() {
            let mut values = args.iter().copied().map(value).collect::<Vec<_>>();
            values[index] = Value::Null;
            assert_eq!(
                function.apply(&values),
                Err(EvalError::InvalidNullConversion)
            );
            values[index] = Value::Text("1".into());
            assert_eq!(function.apply(&values), Err(EvalError::TypeMismatch));
            values[index] = value(f64::INFINITY);
            assert_eq!(function.apply(&values), Err(EvalError::NonFinite));
        }
    }
    near(
        run(FinancialFunction::SLN, &[f64::MAX, -f64::MAX, 2.]).unwrap(),
        f64::MAX,
    );
    let actual = run(FinancialFunction::SYD, &[f64::MAX, 0., f64::MAX, f64::MAX]).unwrap();
    assert!(actual > 0.);
    assert!((actual / (2. / f64::MAX) - 1.).abs() < 1e-12);
    assert_eq!(
        run(
            FinancialFunction::DDB,
            &[f64::MAX, 0., f64::MAX, 1., f64::from_bits(1)]
        )
        .unwrap()
        .to_bits(),
        1
    );
    for (function, args) in [
        (FinancialFunction::SLN, vec![1., 0., 1., 0.]),
        (FinancialFunction::SYD, vec![1., 0., 1.]),
        (FinancialFunction::DDB, vec![1., 0., 1.]),
        (FinancialFunction::DDB, vec![1., 0., 1., 1., 2., 0.]),
    ] {
        assert_eq!(run(function, &args), Err(EvalError::UnsupportedExpression));
    }
}
proptest! {
    #![proptest_config(ProptestConfig {cases:256,rng_seed:proptest::test_runner::RngSeed::Fixed(0x44444231),..Default::default()})]
    #[test]
    fn ordinary_declining_schedules_match_a_separate_book_value_ledger(cost in 100_u32..100000, salvage in 0_u32..100, life in 3_u32..80, factor in prop::sample::select(vec![1.0,1.5,2.0,2.5])) {
        let cost=f64::from(cost);let salvage=f64::from(salvage);let mut book=cost;
        for period in 1..=life {
            let charge=(book*factor/f64::from(life)).min(book-salvage).max(0.);
            let actual=run(FinancialFunction::DDB,&[cost,salvage,f64::from(life),f64::from(period),factor]).unwrap();
            prop_assert!((actual-charge).abs()<cost*2e-12,"{actual} != {charge}, {period}");
            book-=charge;
        }
    }
    #[test]
    fn weighted_year_digits_and_straight_line_conserve_the_depreciable_basis(cost in 100_u32..100000, salvage in 0_u32..100, life in 1_u32..80) {
        let basis=f64::from(cost-salvage);let sum=(1..=life).sum::<u32>();let mut total=0.;
        for period in 1..=life {
            let expected=basis*f64::from(life-period+1)/f64::from(sum);
            let actual=run(FinancialFunction::SYD,&[f64::from(cost),f64::from(salvage),f64::from(life),f64::from(period)]).unwrap();
            prop_assert!((actual-expected).abs()<basis.max(1.)*2e-12);total+=actual;
        }
        prop_assert!((total-basis).abs()<basis.max(1.)*2e-12);
        let straight=run(FinancialFunction::SLN,&[f64::from(cost),f64::from(salvage),f64::from(life)]).unwrap();
        prop_assert!((straight*f64::from(life)-basis).abs()<basis.max(1.)*2e-12);
    }
}
#[cfg(feature = "sql")]
#[test]
fn scalar_depreciation_syntax_keeps_required_slots_and_factor_defaults() {
    use crate::evaluation::{EvalLimits, Evaluator, NoBindings};
    use tablecraft_core::limits::ResourceBudget;
    use tablecraft_sql::{ParseLimits, parse_expression};
    let evaluate = |sql: &str| {
        let expr = parse_expression(sql, ParseLimits::default()).unwrap();
        Evaluator::new(
            &NoBindings,
            &mut ResourceBudget::new(65536, 0, 1024),
            EvalLimits::default(),
        )
        .evaluate(&expr)
    };
    for (sql, expected) in [
        ("SLN(1200,200,4)", 250.),
        ("SYD(1200,200,4,1)", 400.),
        ("DDB(1200,200,4,1)", 600.),
        ("DDB(1200,200,4,1,)", 600.),
        ("DDB(1200,200,4,1,1)", 300.),
    ] {
        assert_eq!(evaluate(sql), Ok(value(expected)), "{sql}");
    }
    assert_eq!(
        evaluate("DDB(Null,200,4,1,1/0)"),
        Err(EvalError::DivisionByZero)
    );
    assert_eq!(
        evaluate("DDB(1200,200,4,1,Null)"),
        Err(EvalError::InvalidNullConversion)
    );
    for sql in [
        "SLN(1200,200,4,)",
        "SYD(1200,200,4,)",
        "DDB(1200,,4,1)",
        "DDB(1200,200,4)",
        "DDB(1200,200,4,1,2,0)",
    ] {
        assert_eq!(
            evaluate(sql),
            Err(EvalError::UnsupportedExpression),
            "{sql}"
        );
    }
}

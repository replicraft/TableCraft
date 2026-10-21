use super::*;
use std::cell::Cell;

#[test]
fn rate_solves_common_zero_negative_positive_and_multiple_root_schedules() {
    for (arguments, expected) in [
        (vec![12., -100., 1000.], 0.0292285407691337),
        (vec![10., -100., 1000.], 0.),
        (vec![2., -2.3, 1., 3.62], 0.1),
        (vec![2., -2.3, 1., 3.62, 0., 0.05], 0.1),
        (vec![2., -2.3, 1., 3.62, 0., 0.25], 0.2),
        (vec![1., -110., 100., 0., 0., 0.], 0.1),
        (vec![1., 0., 100., -95., 0., 0.], -0.05),
    ] {
        let actual = run(FinancialFunction::Rate, &arguments).unwrap();
        assert!(
            (actual - expected).abs() < 1e-7,
            "{arguments:?}: {actual} != {expected}"
        );
    }
    for (rate, periods, begin) in [
        (0.01, 360., 0.),
        (-0.05, 60., 0.),
        (0.1, 60., 1.),
        (1e-12, 12., 1.),
    ] {
        let payment = run(FinancialFunction::Pmt, &[rate, periods, 1000., 0., begin]).unwrap();
        let actual = run(
            FinancialFunction::Rate,
            &[periods, payment, 1000., 0., begin],
        )
        .unwrap();
        assert!((actual - rate).abs() < 1e-7, "{rate} -> {actual}");
    }
}
#[test]
fn invalid_ambiguous_and_unconverged_targets_do_not_return_last_iterates() {
    for args in [
        vec![0., -100., 1000.],
        vec![12., 100., 1000.],
        vec![12., 0., 0.],
        vec![1., -100., 100., 0., 1.],
        vec![12., -100., 1000., 0., 0., -1.],
    ] {
        assert_eq!(
            run(FinancialFunction::Rate, &args),
            Err(EvalError::Domain),
            "{args:?}"
        );
    }
    assert_eq!(
        run(FinancialFunction::Rate, &[12., -100., 1000., 0., 0., 1e100]),
        Err(EvalError::Convergence)
    );
    assert_eq!(
        run(FinancialFunction::Rate, &[12., -1e-300, 1e300]),
        Err(EvalError::PrecisionLoss)
    );
    for index in 0..6 {
        let mut args = vec![
            value(12.),
            value(-100.),
            value(1000.),
            value(0.),
            value(0.),
            value(0.1),
        ];
        args[index] = Value::Null;
        assert_eq!(
            FinancialFunction::Rate.apply(&args),
            Err(EvalError::InvalidNullConversion)
        );
        args[index] = Value::Text("1".into());
        assert_eq!(
            FinancialFunction::Rate.apply(&args),
            Err(EvalError::TypeMismatch)
        );
        args[index] = value(f64::NAN);
        assert_eq!(
            FinancialFunction::Rate.apply(&args),
            Err(EvalError::NonFinite)
        );
    }
}
#[test]
fn every_iteration_and_publication_checkpoint_can_fail_without_retries_or_partial_values() {
    let args = [value(12.), value(-100.), value(1000.)];
    let calls = Cell::new(0);
    FinancialFunction::Rate
        .apply_with_check(&args, &mut || {
            calls.set(calls.get() + 1);
            Ok(())
        })
        .unwrap();
    assert!(calls.get() > 2 && calls.get() <= 21);
    for stop in 1..=calls.get() {
        let count = Cell::new(0);
        assert_eq!(
            FinancialFunction::Rate.apply_with_check(&args, &mut || {
                count.set(count.get() + 1);
                if count.get() == stop {
                    Err(EvalError::Cancelled)
                } else {
                    Ok(())
                }
            }),
            Err(EvalError::Cancelled)
        );
        assert_eq!(count.get(), stop);
    }
    let count = Cell::new(0);
    let args = [
        value(12.),
        value(-100.),
        value(1000.),
        value(0.),
        value(0.),
        value(1e100),
    ];
    assert_eq!(
        FinancialFunction::Rate.apply_with_check(&args, &mut || {
            count.set(count.get() + 1);
            Ok(())
        }),
        Err(EvalError::Convergence)
    );
    assert_eq!(count.get(), 21);
}
proptest! {
    #![proptest_config(ProptestConfig{cases:256,rng_seed:proptest::test_runner::RngSeed::Fixed(0x52415445),..Default::default()})]
    #[test]
    fn rate_recovers_independent_ordinary_cashflow_ledgers(rate in -0.08_f64..0.15, periods in 2_u32..120, present in 100_f64..10000., begin in any::<bool>()) {
        let mut growth=1.;let mut accumulation=0.;
        for _ in 0..periods {growth*=1.+rate;if begin {accumulation+=1.;}accumulation*=1.+rate;if !begin {accumulation+=1.;}}
        let payment=-present*growth/accumulation;
        let actual=run(FinancialFunction::Rate,&[f64::from(periods),payment,present,0.,f64::from(u8::from(begin))]).unwrap();
        prop_assert!((actual-rate).abs()<1e-7,"{actual} != {rate}, n={periods}, begin={begin}");
    }
}
#[cfg(feature = "sql")]
#[test]
fn rate_sql_defaults_optional_slots_and_eager_errors_share_checked_iteration() {
    use crate::evaluation::{EvalLimits, Evaluator, NoBindings};
    use tablecraft_core::limits::ResourceBudget;
    use tablecraft_sql::{ParseLimits, parse_expression};
    let evaluate = |sql: &str, steps| {
        let expr = parse_expression(sql, ParseLimits::default()).unwrap();
        Evaluator::new(
            &NoBindings,
            &mut ResourceBudget::new(65536, 0, steps),
            EvalLimits::default(),
        )
        .evaluate(&expr)
    };
    let first = evaluate("Rate(12,-100,1000)", 1024).unwrap();
    for sql in [
        "Rate(12,-100,1000,,,)",
        "Rate(12,-100,1000,,,0.1)",
        "Rate(12,-100,1000,0,0)",
    ] {
        assert_eq!(evaluate(sql, 1024).unwrap(), first, "{sql}");
    }
    assert!(matches!(
        evaluate("Rate(12,-100,1000)", 5),
        Err(EvalError::Limit(_))
    ));
    assert_eq!(
        evaluate("Rate(Null,-100,1000,,,1/0)", 1024),
        Err(EvalError::DivisionByZero)
    );
    assert_eq!(
        evaluate("Rate(12,-100,1000,,,Null)", 1024),
        Err(EvalError::InvalidNullConversion)
    );
}

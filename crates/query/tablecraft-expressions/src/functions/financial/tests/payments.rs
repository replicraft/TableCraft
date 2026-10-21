use super::*;
#[test]
fn first_last_beginning_zero_rate_and_period_ranges_retain_payment_identity() {
    near(
        run(FinancialFunction::IPmt, &[0.1, 60., 60., 1000., 0., 1.]).unwrap(),
        -8.291694977314386,
    );
    near(
        run(FinancialFunction::IPmt, &[0.01, 1., 12., 1000.]).unwrap(),
        -10.,
    );
    near(
        run(FinancialFunction::PPmt, &[0.01, 1., 12., 1000.]).unwrap(),
        -78.8487886783417,
    );
    for function in [FinancialFunction::IPmt, FinancialFunction::PPmt] {
        for per in [0., -1., 13.] {
            assert_eq!(
                run(function, &[0.01, per, 12., 1000.]),
                Err(EvalError::Domain)
            );
        }
        assert_eq!(
            run(function, &[0.01, 1., 0., 1000.]),
            Err(EvalError::Domain)
        );
        assert_eq!(
            run(function, &[0.01, 1., 12.]),
            Err(EvalError::UnsupportedExpression)
        );
        assert_eq!(
            function.apply(&[
                value(0.01),
                value(1.),
                value(12.),
                value(1000.),
                value(0.),
                Value::Null
            ]),
            Err(EvalError::InvalidNullConversion)
        );
        assert_eq!(
            run(function, &[0.01, 1., 12., 1000., 0., 2.]),
            Err(EvalError::Domain)
        );
    }
    near(
        run(FinancialFunction::IPmt, &[0.01, 1., 12., 1000., 0., 1.]).unwrap(),
        0.,
    );
    near(
        run(FinancialFunction::PPmt, &[0.01, 1., 12., 1000., 0., 1.]).unwrap(),
        run(FinancialFunction::Pmt, &[0.01, 12., 1000., 0., 1.]).unwrap(),
    );
    for per in [1., 12.] {
        near(
            run(FinancialFunction::IPmt, &[0., per, 12., 1200.]).unwrap(),
            0.,
        );
        near(
            run(FinancialFunction::PPmt, &[0., per, 12., 1200.]).unwrap(),
            -100.,
        );
    }
}
proptest! {
    #![proptest_config(ProptestConfig { cases: 256, rng_seed: proptest::test_runner::RngSeed::Fixed(0x49504d54), ..ProptestConfig::default() })]
    #[test]
    fn components_match_an_independent_amortization_ledger(rate in -0.08_f64..0.12, periods in 1_u32..60, present in 100_f64..10000., future in -1000_f64..1000., begin in any::<bool>()) {
        let timing=f64::from(u8::from(begin));let n=f64::from(periods);
        let payment=run(FinancialFunction::Pmt,&[rate,n,present,future,timing]).unwrap();
        let mut balance=present;let mut prior_interest=0.;
        for per in 1..=periods {
            let interest=if begin {if per==1 {0.} else {-prior_interest}} else {-balance*rate};
            let actual_i=run(FinancialFunction::IPmt,&[rate,f64::from(per),n,present,future,timing]).unwrap();
            let actual_p=run(FinancialFunction::PPmt,&[rate,f64::from(per),n,present,future,timing]).unwrap();
            let scale=balance.abs().max(present.abs()).max(payment.abs()).max(1.);
            prop_assert!((actual_i-interest).abs()<scale*2e-11,"{actual_i} != {interest}");
            prop_assert!((actual_p+actual_i-payment).abs()<scale*2e-13);
            if begin {balance+=payment;prior_interest=balance*rate;balance+=prior_interest;} else {balance=balance*(1.+rate)+payment;}
        }
    }
}
#[cfg(feature = "sql")]
#[test]
fn component_sql_required_positions_optional_slots_and_eager_errors_are_shared() {
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
    for sql in ["IPmt(0,1,12,1200)", "IPmt(.01,1,12,1000,,1)"] {
        assert_eq!(evaluate(sql), Ok(value(0.)));
    }
    assert_eq!(evaluate("PPmt(0,1,12,1200,,)"), Ok(value(-100.)));
    assert_eq!(evaluate("NPer(0,-100,1000,,1)"), Ok(value(10.)));
    assert_eq!(
        evaluate("IPmt(Null,1,12,1000,1/0)"),
        Err(EvalError::DivisionByZero)
    );
    for sql in [
        "IPmt(.01,1,12)",
        "PPmt(.01,,12,1000)",
        "NPer(0,-100,1000,0,0,0)",
    ] {
        assert_eq!(evaluate(sql), Err(EvalError::UnsupportedExpression));
    }
}

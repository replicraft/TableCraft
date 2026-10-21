use super::*;
#[test]
fn period_solver_zero_rate_repayment_growth_and_impossible_targets_are_explicit() {
    near(
        run(FinancialFunction::NPer, &[0., -100., 1000.]).unwrap(),
        10.,
    );
    near(
        run(FinancialFunction::NPer, &[0.01, -100., 1000.]).unwrap(),
        10.58864445942323,
    );
    near(
        run(FinancialFunction::NPer, &[0.01, 0., 100., -200.]).unwrap(),
        2_f64.ln() / 1.01_f64.ln(),
    );
    near(
        run(FinancialFunction::NPer, &[0., f64::MAX, f64::MAX, f64::MAX]).unwrap(),
        -2.,
    );
    for rate in [1e-20, -1e-20, f64::from_bits(1), -f64::from_bits(1)] {
        near(
            run(FinancialFunction::NPer, &[rate, -100., 1000.]).unwrap(),
            10.,
        );
    }
    for arguments in [
        &[0.01, -1., 1000.][..],
        &[0.01, -10., 1000.],
        &[-1., -100., 1000.],
        &[-2., -100., 1000.],
        &[0., 0., 0.],
    ] {
        assert_eq!(
            run(FinancialFunction::NPer, arguments),
            Err(EvalError::Domain),
            "{arguments:?}"
        );
    }
    assert_eq!(
        run(FinancialFunction::NPer, &[0., 0., 1000.]),
        Err(EvalError::DivisionByZero)
    );
    assert_eq!(
        run(FinancialFunction::NPer, &[f64::MAX, 1., 1., 0., 1.]),
        Err(EvalError::Overflow)
    );
}
proptest! {
    #![proptest_config(ProptestConfig { cases: 256, rng_seed: proptest::test_runner::RngSeed::Fixed(0x4e504552), ..ProptestConfig::default() })]
    #[test]
    fn solved_period_count_recovers_independently_stepped_cashflows(rate in -0.08_f64..0.12, periods in 1_u32..80, payment in -1000_f64..-100., present in 100_f64..10000., begin in any::<bool>()) {
        let mut balance=present;
        for _ in 0..periods {
            if begin {balance+=payment;}
            balance*=1.+rate;
            if !begin {balance+=payment;}
        }
        // Restrict this inversion property to identifiable, well-conditioned
        // schedules. Exact interest-only ambiguity has a separate Domain control.
        prop_assume!((present*rate+payment*(1.+rate*f64::from(u8::from(begin)))).abs() > 1e-5);
        let result=run(FinancialFunction::NPer,&[rate,payment,present,-balance,f64::from(u8::from(begin))]).unwrap();
        prop_assert!((result-f64::from(periods)).abs()<1e-6,"{result} != {periods}");
    }
}

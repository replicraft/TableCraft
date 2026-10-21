use super::*;
use proptest::prelude::*;
use std::cell::Cell;
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Double, Value},
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(4 * 1024 * 1024, 0, 1_000_000)
}

fn values(numbers: &[f64]) -> Vec<Value> {
    numbers
        .iter()
        .map(|&v| Value::Double(Double::from_float(v)))
        .collect()
}

fn flows(numbers: &[f64]) -> CashFlows {
    CashFlows::new(&values(numbers), &mut budget(), None).unwrap()
}

fn near(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 2e-11 * expected.abs().max(1.0),
        "{actual} != {expected}"
    );
}

#[test]
fn npv_preserves_period_one_timing_zero_positions_and_single_sign_series() {
    near(flows(&[110.]).npv(0.1, &mut budget(), None).unwrap(), 100.);
    near(
        flows(&[0., 121.]).npv(0.1, &mut budget(), None).unwrap(),
        100.,
    );
    near(
        flows(&[-100., 0., 125.])
            .npv(0., &mut budget(), None)
            .unwrap(),
        25.,
    );
    near(
        flows(&[1e16, 1., -1e16])
            .npv(0., &mut budget(), None)
            .unwrap(),
        1.,
    );
    near(flows(&[1., 2.]).npv(-2., &mut budget(), None).unwrap(), 1.);
    near(
        flows(&[0., 0.]).npv(f64::MAX, &mut budget(), None).unwrap(),
        0.,
    );

    assert_eq!(
        flows(&[]).npv(0., &mut budget(), None),
        Err(EvalError::Domain)
    );
    assert_eq!(
        flows(&[1.]).npv(-1., &mut budget(), None),
        Err(EvalError::Domain)
    );
    assert_eq!(
        flows(&[1.]).npv(f64::NAN, &mut budget(), None),
        Err(EvalError::NonFinite)
    );
    assert_eq!(
        flows(&[f64::MAX, 1e-300]).npv(0., &mut budget(), None),
        Err(EvalError::PrecisionLoss)
    );
}

#[test]
fn irr_handles_zero_negative_and_guess_selected_multiple_roots() {
    for (numbers, guess, expected) in [
        (vec![-100., 110.], None, 0.1),
        (vec![-100., 0., 121.], None, 0.1),
        (vec![-100., 100.], None, 0.),
        (vec![-100., 95.], None, -0.05),
        (vec![-100., 230., -132.], Some(0.05), 0.1),
        (vec![-100., 230., -132.], Some(0.25), 0.2),
    ] {
        near(
            flows(&numbers).irr(guess, &mut budget(), None).unwrap(),
            expected,
        );
    }

    assert_eq!(
        flows(&[0., 1.]).irr(None, &mut budget(), None),
        Err(EvalError::Domain)
    );
    assert_eq!(
        flows(&[-1., 1.]).irr(Some(-1.), &mut budget(), None),
        Err(EvalError::Domain)
    );
    assert_eq!(
        flows(&[-1., 1.]).irr(Some(1e100), &mut budget(), None),
        Err(EvalError::Convergence)
    );
    assert_eq!(
        flows(&[-1., 1., -1.]).irr(None, &mut budget(), None),
        Err(EvalError::Convergence)
    );
}

#[test]
fn mirr_keeps_trailing_zero_periods_and_scales_extreme_cashflows() {
    near(
        flows(&[-100., 110.])
            .mirr(0.05, 0.1, &mut budget(), None)
            .unwrap(),
        0.1,
    );
    near(
        flows(&[-100., 110., 0.])
            .mirr(0.05, 0.21, &mut budget(), None)
            .unwrap(),
        1.331_f64.sqrt() - 1.,
    );
    near(
        flows(&[-1e300, 1e300])
            .mirr(0.1, 0.1, &mut budget(), None)
            .unwrap(),
        0.,
    );
    near(
        flows(&[-100., 0., 121.])
            .mirr(0., 0., &mut budget(), None)
            .unwrap(),
        0.1,
    );

    for numbers in [&[][..], &[0., 0.], &[1., 2.], &[-1., -2.]] {
        assert_eq!(
            flows(numbers).mirr(0.1, 0.1, &mut budget(), None),
            Err(EvalError::Domain)
        );
    }
    assert_eq!(
        flows(&[-1., 2.]).mirr(-1., 0.1, &mut budget(), None),
        Err(EvalError::Domain)
    );
}

#[test]
fn constructors_enforce_carriers_limits_budgets_and_payload_redaction() {
    for (value, error) in [
        (Value::Null, EvalError::InvalidNullConversion),
        (Value::Text("secret".into()), EvalError::TypeMismatch),
        (values(&[f64::INFINITY]).remove(0), EvalError::NonFinite),
    ] {
        assert_eq!(
            CashFlows::new(&[value], &mut budget(), None).unwrap_err(),
            error
        );
    }
    let input = vec![Value::Int32(123456); 65_537];
    assert_eq!(
        CashFlows::new(&input, &mut budget(), None).unwrap_err(),
        EvalError::ArrayLimit
    );
    let full = CashFlows::new(&input[..65_536], &mut budget(), None).unwrap();
    assert_eq!(full.len(), 65_536);
    assert!(!full.is_empty());

    let mut limited = ResourceBudget::new(7, 0, 100);
    assert!(matches!(
        CashFlows::new(&[Value::Int32(1)], &mut limited, None),
        Err(EvalError::Limit(_))
    ));
    let boolean = CashFlows::new(
        &[Value::Boolean(true), Value::Int32(2)],
        &mut budget(),
        None,
    )
    .unwrap();
    near(boolean.npv(0., &mut budget(), None).unwrap(), 1.);
    assert_eq!(
        format!("{:?}", flows(&[123456.])),
        "CashFlows { len: 1, .. }"
    );
}

#[test]
fn every_cashflow_pass_is_cancellable_and_shares_the_step_budget() {
    let input = flows(&[-100., 0., 121.]);
    let calls = Cell::new(0);
    input
        .irr(
            None,
            &mut budget(),
            Some(&|| {
                calls.set(calls.get() + 1);
                false
            }),
        )
        .unwrap();

    for stop in 1..=calls.get() {
        let count = Cell::new(0);
        assert_eq!(
            input.irr(
                None,
                &mut budget(),
                Some(&|| {
                    count.set(count.get() + 1);
                    count.get() == stop
                })
            ),
            Err(EvalError::Cancelled)
        );
    }

    for operation in [
        CashFlowFunction::NPV,
        CashFlowFunction::IRR,
        CashFlowFunction::MIRR,
    ] {
        let arguments = match operation {
            CashFlowFunction::NPV => values(&[0.1]),
            CashFlowFunction::IRR => vec![],
            CashFlowFunction::MIRR => values(&[0.1, 0.1]),
        };
        let mut limited = ResourceBudget::new(1_000_000, 0, 4);
        assert!(matches!(
            operation.apply(&arguments, &values(&[-100., 0., 121.]), &mut limited, None),
            Err(EvalError::Limit(_))
        ));
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 256,
        rng_seed: proptest::test_runner::RngSeed::Fixed(0x43415348464c4f57),
        ..ProptestConfig::default()
    })]

    #[test]
    fn kernels_match_independent_discount_and_reinvestment_ledgers(
        entries in prop::collection::vec(-10_000_i32..10_000, 2..20),
        basis_points in -200_i32..2000,
        finance_points in 0_i32..2000,
    ) {
        let numbers: Vec<f64> = entries.iter().map(|&v| f64::from(v)).collect();
        let rate = f64::from(basis_points) / 10_000.;
        let finance = f64::from(finance_points) / 10_000.;
        let input = flows(&numbers);
        let growth = 1. + rate;
        let mut discount = growth;
        let mut present = 0.;
        let mut negative = 0.;
        let mut positive = 0.;
        let end = numbers.len() - 1;

        for (index, &value) in numbers.iter().enumerate() {
            present += value / discount;
            discount *= growth;
            if value < 0. {
                negative -= value / (1. + finance).powi(index as i32);
            } else {
                positive += value * growth.powi((end - index) as i32);
            }
        }

        near(input.npv(rate, &mut budget(), None).unwrap(), present);
        if positive > 0. && negative > 0. {
            let expected = (positive / negative).powf(1. / end as f64) - 1.;
            near(input.mirr(finance, rate, &mut budget(), None).unwrap(), expected);
        }
    }

    #[test]
    fn irr_matches_a_known_root_without_reusing_the_solver(
        period in 1_i32..20,
        rate_points in -100_i32..2000,
        principal in 1_i32..10000,
    ) {
        let expected = f64::from(rate_points) / 10_000.;
        let mut numbers = vec![0.; period as usize + 1];
        numbers[0] = -f64::from(principal);
        numbers[period as usize] = f64::from(principal) * (1. + expected).powi(period);

        near(flows(&numbers).irr(None, &mut budget(), None).unwrap(), expected);
    }
}

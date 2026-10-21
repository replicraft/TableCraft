#[path = "tests/depreciation.rs"]
mod depreciation;
#[path = "tests/payments.rs"]
mod payments;
#[path = "tests/periods.rs"]
mod periods;
use super::*;
use proptest::prelude::*;
use tablecraft_core::values::{Currency, Decimal, Single};
fn value(number: f64) -> Value {
    Value::Double(Double::from_float(number))
}
fn run(function: FinancialFunction, numbers: &[f64]) -> Result<f64, EvalError> {
    let Value::Double(result) =
        function.apply(&numbers.iter().copied().map(value).collect::<Vec<_>>())?
    else {
        panic!("Double required")
    };
    Ok(result.to_float())
}
fn near(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= expected.abs().max(1.) * 2e-12,
        "{actual} != {expected}"
    );
}
#[test]
fn published_annuity_shapes_and_cashflow_directions_use_double_results() {
    near(
        run(FinancialFunction::Pmt, &[0.01, 12., 1000.]).unwrap(),
        -88.8487886783417,
    );
    near(
        run(FinancialFunction::PV, &[0.01, 12., -100.]).unwrap(),
        1125.507747348463,
    );
    near(
        run(FinancialFunction::FV, &[0.01, 12., -100.]).unwrap(),
        1268.250301319697,
    );
    near(
        run(FinancialFunction::PV, &[0.0825, 20., -50000., 1000000., 1.]).unwrap(),
        316811.95778831956,
    );
    for function in [
        FinancialFunction::Pmt,
        FinancialFunction::PV,
        FinancialFunction::FV,
    ] {
        near(
            run(function, &[0., 10., 100., 200., 0.]).unwrap(),
            if function == FinancialFunction::Pmt {
                -30.
            } else {
                -1200.
            },
        );
        let end = run(function, &[0.02, 24., 100., 0., 0.]).unwrap();
        let begin = run(function, &[0.02, 24., 100., 0., 1.]).unwrap();
        near(
            begin,
            if function == FinancialFunction::Pmt {
                end / 1.02
            } else {
                end * 1.02
            },
        );
    }
}
#[test]
fn numeric_carriers_null_types_timing_and_nonfinite_inputs_have_explicit_errors() {
    near(
        run(FinancialFunction::Pmt, &[0.01, 3., f64::MAX, f64::MAX]).unwrap(),
        -f64::MAX * (2.030301 / 3.0301),
    );
    for rate in [
        Value::Byte(0),
        Value::Int16(0),
        Value::Int32(0),
        Value::Int64(0),
        Value::Single(Single::from_float(0.)),
        value(0.),
        Value::Currency(Currency::from_scaled(0)),
        Value::Decimal(Decimal::new(0, 4).unwrap()),
        Value::Boolean(false),
    ] {
        assert_eq!(
            FinancialFunction::Pmt.apply(&[rate, Value::Int32(10), Value::Int32(100)]),
            Ok(value(-10.))
        );
    }
    for function in [
        FinancialFunction::Pmt,
        FinancialFunction::PV,
        FinancialFunction::FV,
    ] {
        for index in 0..5 {
            let mut arguments = vec![value(0.), value(12.), value(100.), value(0.), value(0.)];
            arguments[index] = Value::Null;
            assert_eq!(
                function.apply(&arguments),
                Err(EvalError::InvalidNullConversion)
            );
            arguments[index] = Value::Text("1".into());
            assert_eq!(function.apply(&arguments), Err(EvalError::TypeMismatch));
            for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
                arguments[index] = value(bad);
                assert_eq!(function.apply(&arguments), Err(EvalError::NonFinite));
            }
        }
        assert_eq!(run(function, &[0., 1., 1., 0., 2.]), Err(EvalError::Domain));
        assert_eq!(
            run(function, &[0., 1., 1., 0., 0.5]),
            Err(EvalError::Domain)
        );
        for n in [0, 1, 2, 6] {
            assert_eq!(
                function.apply(&vec![value(0.); n]),
                Err(EvalError::UnsupportedExpression)
            );
        }
    }
}
#[test]
fn zero_small_and_negative_rates_and_finite_range_boundaries_remain_distinct() {
    assert_eq!(
        run(FinancialFunction::Pmt, &[0., 0., 1.]),
        Err(EvalError::DivisionByZero)
    );
    assert_eq!(
        run(FinancialFunction::Pmt, &[0.1, 0., 1.]),
        Err(EvalError::DivisionByZero)
    );
    near(
        run(FinancialFunction::FV, &[0., 0., 1., 42.]).unwrap(),
        -42.,
    );
    near(
        run(FinancialFunction::PV, &[0., 0., 1., 42.]).unwrap(),
        -42.,
    );
    near(
        run(FinancialFunction::Pmt, &[0., 2., f64::MAX, f64::MAX]).unwrap(),
        -f64::MAX,
    );
    for rate in [1e-20, -1e-20, f64::from_bits(1), -f64::from_bits(1)] {
        near(
            run(FinancialFunction::FV, &[rate, 0.25, -100.]).unwrap(),
            25.,
        );
        near(
            run(FinancialFunction::Pmt, &[rate, 0.25, 100.]).unwrap(),
            -400.,
        );
    }
    near(
        run(FinancialFunction::FV, &[-2., 2., 10., 20.]).unwrap(),
        -20.,
    );
    assert_eq!(
        run(FinancialFunction::FV, &[-2., 0.5, 10.]),
        Err(EvalError::Domain)
    );
    near(
        run(FinancialFunction::PV, &[1., 2000., -100.]).unwrap(),
        100.,
    );
    near(
        run(FinancialFunction::Pmt, &[1., 2000., 100.]).unwrap(),
        -100.,
    );
    assert_eq!(
        run(FinancialFunction::FV, &[1., 2000., -100.]),
        Err(EvalError::Overflow)
    );
    assert_eq!(
        run(FinancialFunction::PV, &[-1., 1., 10.]),
        Err(EvalError::DivisionByZero)
    );
}
proptest! {
    #![proptest_config(ProptestConfig { cases: 256, rng_seed: proptest::test_runner::RngSeed::Fixed(0x414e4e55), ..ProptestConfig::default() })]
    #[test]
    fn integer_period_future_values_match_an_independent_period_by_period_ledger(rate in -0.1_f64..0.15, periods in 1_u32..80, payment in -10000_f64..10000., present in -10000_f64..10000., begin in any::<bool>()) {
        let mut ledger=present;
        for _ in 0..periods {
            if begin {ledger+=payment;}
            ledger*=1.+rate;
            if !begin {ledger+=payment;}
        }
        let result=run(FinancialFunction::FV,&[rate,f64::from(periods),payment,present,f64::from(u8::from(begin))]).unwrap();
        prop_assert!((result+ledger).abs()<=ledger.abs().max(payment.abs()).max(present.abs()).max(1.)*1e-11,"{result} != {}",-ledger);
    }
}

#[cfg(feature = "sql")]
#[test]
fn scalar_sql_defaults_omitted_slots_and_eager_argument_errors_use_the_same_kernel() {
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
        ("Pmt(0,10,100)", -10.),
        ("PV(0,10,-100)", 1000.),
        ("FV(0,10,-100)", 1000.),
        ("Pmt(0,10,100,,)", -10.),
        ("Pmt(0,10,100,,1)", -10.),
    ] {
        assert_eq!(evaluate(sql), Ok(value(expected)));
    }
    assert_eq!(
        evaluate("Pmt(Null,10,100,1/0)"),
        Err(EvalError::DivisionByZero)
    );
    for sql in [
        "Pmt(0,10)",
        "PV(0,,100)",
        "FV(*,10,100)",
        "Module.Pmt(0,10,100)",
        "Pmt(0,10,100,0,0,0)",
    ] {
        assert_eq!(
            evaluate(sql),
            Err(EvalError::UnsupportedExpression),
            "{sql}"
        );
    }
    assert_eq!(
        evaluate("FV(0,10,100,Null)"),
        Err(EvalError::InvalidNullConversion)
    );
}

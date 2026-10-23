use super::{EvalLimits, Evaluator, NoBindings};
use crate::{
    EvalError,
    functions::{
        conversions::text::NumberFormat,
        math::{MathFunction, random::RndState},
    },
};
use std::cell::Cell;
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Decimal, Double, Value},
};

#[test]
fn explicit_math_text_uses_double_long_and_single_targets_without_erasing_fixed_carriers() {
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .with_number_format(NumberFormat::DeDe);
    for (function, input, expected) in [
        (MathFunction::Abs, "-2,5", 2.5),
        (MathFunction::Int, "-2,5", -3.0),
        (MathFunction::Fix, "-2,5", -2.0),
        (MathFunction::Sqr, "2,25", 1.5),
        (MathFunction::Exp, "0", 1.0),
        (MathFunction::Log, "1", 0.0),
        (MathFunction::Sin, "0", 0.0),
        (MathFunction::Cos, "0", 1.0),
        (MathFunction::Tan, "0", 0.0),
        (MathFunction::Atn, "0", 0.0),
    ] {
        assert_eq!(
            evaluator.apply_math(function, &[Value::Text(input.into())]),
            Ok(Value::Double(Double::from_float(expected)))
        );
    }
    assert_eq!(
        evaluator.apply_math(MathFunction::Sgn, &[Value::Text("-0,5".into())]),
        Ok(Value::Int16(-1))
    );
    assert_eq!(
        evaluator.apply_math(
            MathFunction::Rgb,
            &[
                Value::Text("2,5".into()),
                Value::Text("1,5".into()),
                Value::Text("3,5".into())
            ]
        ),
        Ok(Value::Int32(262658))
    );
    assert_eq!(
        evaluator.apply_math(MathFunction::QbColor, &[Value::Text("2,5".into())]),
        Ok(Value::Int32(32768))
    );
    assert_eq!(
        evaluator.apply_math(
            MathFunction::Round,
            &[
                Value::Decimal(Decimal::new(2675, 3).unwrap()),
                Value::Text("2".into())
            ]
        ),
        Ok(Value::Decimal(Decimal::new(268, 2).unwrap()))
    );
}

#[test]
fn contextual_math_preserves_validation_order_and_failed_rng_controls_do_not_advance_state() {
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    assert_eq!(
        evaluator.apply_math(
            MathFunction::Rgb,
            &[Value::Int16(-1), Value::Text("bad".into()), Value::Byte(0)]
        ),
        Err(EvalError::Domain)
    );
    assert_eq!(
        evaluator.apply_math(MathFunction::Abs, &[Value::Text("1".into())]),
        Err(EvalError::NumberFormatRequired)
    );
    assert_eq!(
        evaluator.apply_math(MathFunction::Rnd, &[Value::Text("bad".into())]),
        Err(EvalError::RandomStateRequired)
    );
    let state = RndState::default();
    let expected = RndState::default();
    let mut evaluator = evaluator
        .with_random(&state)
        .with_number_format(NumberFormat::EnUs);
    assert_eq!(
        evaluator.apply_math(
            MathFunction::Round,
            &[Value::Text("bad".into()), Value::Text("29".into())]
        ),
        Err(EvalError::Domain)
    );
    assert_eq!(
        evaluator.apply_math(MathFunction::Rnd, &[Value::Text("bad".into())]),
        Err(EvalError::TypeMismatch)
    );
    assert_eq!(state.apply(None), expected.apply(None));
    assert_eq!(
        evaluator.apply_math(MathFunction::Rnd, &[Value::Text("-1".into())]),
        expected.apply(Some(&Value::Int16(-1)))
    );
}

#[test]
fn math_text_paths_share_limits_and_cancellation_checkpoints() {
    for (function, values) in [
        (MathFunction::Abs, vec![Value::Text("-2.5".into())]),
        (
            MathFunction::Round,
            vec![Value::Text("2.5".into()), Value::Text("0".into())],
        ),
        (
            MathFunction::Rgb,
            vec![
                Value::Text("1".into()),
                Value::Text("2".into()),
                Value::Text("3".into()),
            ],
        ),
        (MathFunction::Rnd, vec![Value::Text("-1".into())]),
    ] {
        let checks = Cell::new(0);
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        let state = RndState::default();
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_number_format(NumberFormat::EnUs)
            .with_random(&state)
            .with_cancellation(&|| {
                checks.set(checks.get() + 1);
                false
            })
            .apply_math(function, &values)
            .unwrap();
        for stop in 1..=checks.get() {
            let calls = Cell::new(0);
            let mut budget = ResourceBudget::new(65536, 0, 10000);
            let state = RndState::default();
            let result = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_number_format(NumberFormat::EnUs)
                .with_random(&state)
                .with_cancellation(&|| {
                    calls.set(calls.get() + 1);
                    calls.get() >= stop
                })
                .apply_math(function, &values);
            assert_eq!(
                result,
                Err(EvalError::Cancelled),
                "{function:?} checkpoint {stop}"
            );
        }
    }
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    assert_eq!(
        Evaluator::new(
            &NoBindings,
            &mut budget,
            EvalLimits {
                max_text_bytes: 2,
                ..Default::default()
            }
        )
        .with_number_format(NumberFormat::EnUs)
        .apply_math(MathFunction::Abs, &[Value::Text("2.5".into())]),
        Err(EvalError::TextLimit)
    );
}

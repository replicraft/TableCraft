use super::*;
use crate::{
    evaluation::{EvalLimits, NoBindings},
    functions::{conversions::text::NumberFormat, math::random::RndState},
};
use std::cell::Cell;
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Double, ExpressionString, Single},
};

fn retained(text: &str) -> ExpressionValue {
    let mut bytes = vec![0, 216];
    bytes.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
    let end = bytes.len();
    bytes.push(255);
    ExpressionValue::StringBytes(ExpressionString::from_owned(bytes).select(2..end).unwrap())
}

fn cases() -> Vec<(MathFunction, Vec<&'static str>, Value)> {
    vec![
        (MathFunction::Abs, vec!["-2.5"], double(2.5)),
        (MathFunction::Sgn, vec!["-2.5"], Value::Int16(-1)),
        (MathFunction::Int, vec!["-2.5"], double(-3.0)),
        (MathFunction::Fix, vec!["-2.5"], double(-2.0)),
        (MathFunction::Sqr, vec!["4"], double(2.0)),
        (MathFunction::Exp, vec!["0"], double(1.0)),
        (MathFunction::Log, vec!["1"], double(0.0)),
        (MathFunction::Sin, vec!["0"], double(0.0)),
        (MathFunction::Cos, vec!["0"], double(1.0)),
        (MathFunction::Tan, vec!["0"], double(0.0)),
        (MathFunction::Atn, vec!["0"], double(0.0)),
        (MathFunction::Round, vec!["2.5", "0"], double(2.0)),
        (MathFunction::Rgb, vec!["1", "2", "3"], Value::Int32(197121)),
        (MathFunction::QbColor, vec!["2.5"], Value::Int32(32768)),
        (
            MathFunction::Rnd,
            vec!["0"],
            Value::Single(Single::from_float(0.01953125)),
        ),
    ]
}

fn double(value: f64) -> Value {
    Value::Double(Double::from_float(value))
}

#[test]
fn retained_numeric_text_uses_the_same_contextual_math_conversion() {
    for (function, arguments, expected) in cases() {
        for format in [NumberFormat::EnUs, NumberFormat::DeDe] {
            let arguments: Vec<_> = arguments
                .iter()
                .map(|text| {
                    if format == NumberFormat::DeDe {
                        text.replace('.', ",")
                    } else {
                        (*text).to_owned()
                    }
                })
                .collect();
            let materialized: Vec<_> = arguments.iter().cloned().map(Value::Text).collect();
            let raw: Vec<_> = arguments.iter().map(|text| retained(text)).collect();
            let state = RndState::default();
            let mut budget = ResourceBudget::new(65536, 0, 10000);
            let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_number_format(format)
                .with_random(&state);
            assert_eq!(
                evaluator.apply_math(function, &materialized),
                Ok(expected.clone()),
                "materialized {function:?}"
            );
            assert_eq!(
                evaluator.apply_expression_math(function, &raw),
                Ok(expected.clone()),
                "retained {function:?}"
            );
        }
    }
}

fn malformed() -> ExpressionValue {
    ExpressionValue::StringBytes(ExpressionString::from_owned(vec![0, 216]))
}

#[test]
fn progressive_validation_preserves_independently_expected_error_precedence() {
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .with_number_format(NumberFormat::EnUs);
    for (function, arguments, expected) in [
        (
            MathFunction::Abs,
            vec![malformed(), retained("1")],
            EvalError::UnsupportedExpression,
        ),
        (
            MathFunction::Round,
            vec![malformed(), retained("-1")],
            EvalError::Domain,
        ),
        (
            MathFunction::Round,
            vec![malformed(), retained("2147483648")],
            EvalError::Overflow,
        ),
        (
            MathFunction::Round,
            vec![ExpressionValue::Value(Value::Null), malformed()],
            EvalError::UnrepresentableText,
        ),
        (
            MathFunction::Round,
            vec![malformed(), ExpressionValue::Value(Value::Null)],
            EvalError::InvalidNullConversion,
        ),
        (
            MathFunction::Rgb,
            vec![retained("-1"), malformed(), malformed()],
            EvalError::Domain,
        ),
        (
            MathFunction::Rgb,
            vec![malformed(), retained("-1"), malformed()],
            EvalError::UnrepresentableText,
        ),
        (
            MathFunction::QbColor,
            vec![retained("16")],
            EvalError::Domain,
        ),
        (
            MathFunction::Rnd,
            vec![malformed()],
            EvalError::RandomStateRequired,
        ),
    ] {
        assert_eq!(
            evaluator.apply_expression_math(function, &arguments),
            Err(expected),
            "{function:?}"
        );
    }

    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    assert_eq!(
        evaluator.apply_expression_math(MathFunction::Round, &[malformed(), retained("0")]),
        Err(EvalError::NumberFormatRequired)
    );
    assert_eq!(
        evaluator.apply_expression_math(MathFunction::Abs, &[malformed()]),
        Err(EvalError::UnrepresentableText)
    );
    assert_eq!(
        evaluator
            .with_repeatable_dynamic_expressions()
            .apply_expression_math(MathFunction::Rnd, &[malformed()]),
        Err(EvalError::DynamicNotRepeatable)
    );
}

#[test]
fn retained_rnd_converts_before_mutation_and_advances_exactly_once() {
    let state = RndState::default();
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .with_number_format(NumberFormat::EnUs)
        .with_random(&state);
    for bad in [malformed(), retained("not numeric")] {
        assert!(
            evaluator
                .apply_expression_math(MathFunction::Rnd, &[bad])
                .is_err()
        );
        assert_eq!(
            state.apply(Some(&Value::Int16(0))),
            Ok(Value::Single(Single::from_float(0.01953125)))
        );
    }
    // Independent one-step integer recurrence from the published default raw seed.
    let seed = (0x50000_u64 * 0x43fd43fd + 0xc39ec3) % (1 << 24);
    let expected = Value::Single(Single::from_float(seed as f32 / 16777216.0));
    assert_eq!(
        evaluator.apply_expression_math(MathFunction::Rnd, &[retained("1")]),
        Ok(expected.clone())
    );
    assert_eq!(state.apply(Some(&Value::Int16(0))), Ok(expected));
}

#[test]
fn all_retained_math_calls_enforce_active_range_limits_and_cancellation() {
    for (function, arguments, _) in cases() {
        let arguments: Vec<_> = arguments.iter().map(|text| retained(text)).collect();
        let calls = Cell::new(0);
        let state = RndState::default();
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_number_format(NumberFormat::EnUs)
            .with_random(&state)
            .with_cancellation(&|| {
                calls.set(calls.get() + 1);
                false
            })
            .apply_expression_math(function, &arguments)
            .unwrap();
        for stop in 1..=calls.get() {
            let calls = Cell::new(0);
            let state = RndState::default();
            let mut budget = ResourceBudget::new(65536, 0, 10000);
            assert_eq!(
                Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                    .with_number_format(NumberFormat::EnUs)
                    .with_random(&state)
                    .with_cancellation(&|| {
                        calls.set(calls.get() + 1);
                        calls.get() >= stop
                    })
                    .apply_expression_math(function, &arguments),
                Err(EvalError::Cancelled),
                "{function:?} cancellation {stop}"
            );
        }
        for (bytes, steps) in [(0, 10000), (65536, 0)] {
            let mut budget = ResourceBudget::new(bytes, 0, steps);
            assert!(matches!(
                Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                    .with_number_format(NumberFormat::EnUs)
                    .with_random(&state)
                    .apply_expression_math(function, &arguments),
                Err(EvalError::Limit(_))
            ));
        }
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        assert_eq!(
            Evaluator::new(
                &NoBindings,
                &mut budget,
                EvalLimits {
                    max_text_bytes: 0,
                    ..Default::default()
                },
            )
            .with_number_format(NumberFormat::EnUs)
            .with_random(&state)
            .apply_expression_math(function, &arguments),
            Err(EvalError::TextLimit)
        );
    }
}

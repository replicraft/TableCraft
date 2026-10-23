use super::*;
use crate::{
    evaluation::{EvalLimits, NoBindings},
    functions::math::random::RndState,
};
use std::cell::Cell;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_core::values::Double;
use tablecraft_sql::{parse_select, select::Projection};

#[test]
fn math_empty_uses_reviewed_zero_targets_without_creating_stored_states() {
    let state = RndState::default();
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let mut evaluator =
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default()).with_random(&state);
    for (function, expected) in [
        (MathFunction::Abs, Value::Int16(0)),
        (MathFunction::Int, Value::Int16(0)),
        (MathFunction::Fix, Value::Int16(0)),
        (MathFunction::Sgn, Value::Int16(0)),
        (MathFunction::Sqr, Value::Double(Double::from_float(0.0))),
        (MathFunction::Exp, Value::Double(Double::from_float(1.0))),
        (MathFunction::Sin, Value::Double(Double::from_float(0.0))),
        (MathFunction::Cos, Value::Double(Double::from_float(1.0))),
        (MathFunction::Tan, Value::Double(Double::from_float(0.0))),
        (MathFunction::Atn, Value::Double(Double::from_float(0.0))),
        (MathFunction::QbColor, Value::Int32(0)),
    ] {
        assert_eq!(
            evaluator.apply_expression_math(function, &[ExpressionValue::Empty]),
            Ok(expected)
        );
    }
    assert_eq!(
        evaluator.apply_expression_math(MathFunction::Log, &[ExpressionValue::Empty]),
        Err(EvalError::Domain)
    );
    assert_eq!(
        evaluator.apply_expression_math(
            MathFunction::Rgb,
            &[
                ExpressionValue::Empty,
                ExpressionValue::Empty,
                ExpressionValue::Empty
            ]
        ),
        Ok(Value::Int32(0))
    );
    assert_eq!(
        evaluator.apply_expression_math(
            MathFunction::Round,
            &[
                ExpressionValue::Value(Value::Double(Double::from_float(2.5))),
                ExpressionValue::Empty
            ]
        ),
        Ok(Value::Double(Double::from_float(2.0)))
    );
    assert_eq!(
        evaluator.apply_expression_math(MathFunction::Round, &[ExpressionValue::Empty]),
        Err(EvalError::NonScalarExpression)
    );
    let before = state.apply(Some(&Value::Int16(0))).unwrap();
    assert_eq!(
        evaluator.apply_expression_math(MathFunction::Rnd, &[ExpressionValue::Empty]),
        Ok(before)
    );
    assert_eq!(state.apply(None), RndState::default().apply(None));
}

#[test]
fn error_data_is_rejected_at_the_original_kernel_validation_position() {
    let state = RndState::default();
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let mut evaluator =
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default()).with_random(&state);
    for (function, values, expected) in [
        (
            MathFunction::Round,
            vec![
                ExpressionValue::Error(1),
                ExpressionValue::Value(Value::Int16(29)),
            ],
            EvalError::Domain,
        ),
        (
            MathFunction::Round,
            vec![
                ExpressionValue::Error(1),
                ExpressionValue::Value(Value::Null),
            ],
            EvalError::InvalidNullConversion,
        ),
        (
            MathFunction::Round,
            vec![
                ExpressionValue::Value(Value::Null),
                ExpressionValue::Error(1),
            ],
            EvalError::TypeMismatch,
        ),
        (
            MathFunction::Rgb,
            vec![
                ExpressionValue::Value(Value::Int16(-1)),
                ExpressionValue::Error(1),
                ExpressionValue::Empty,
            ],
            EvalError::Domain,
        ),
        (
            MathFunction::Rgb,
            vec![
                ExpressionValue::Error(1),
                ExpressionValue::Value(Value::Int16(-1)),
                ExpressionValue::Empty,
            ],
            EvalError::TypeMismatch,
        ),
        (
            MathFunction::Rnd,
            vec![ExpressionValue::Error(1)],
            EvalError::TypeMismatch,
        ),
        (
            MathFunction::Abs,
            vec![ExpressionValue::Error(u32::MAX)],
            EvalError::TypeMismatch,
        ),
        (
            MathFunction::Fix,
            vec![ExpressionValue::Missing],
            EvalError::NonScalarExpression,
        ),
    ] {
        assert_eq!(
            evaluator.apply_expression_math(function, &values),
            Err(expected)
        );
    }
    assert_eq!(state.apply(None), RndState::default().apply(None));
}

#[test]
fn all_sql_arguments_evaluate_before_error_data_admission() {
    for sql in [
        "SELECT RGB(CVErr(1),1/0,0)",
        "SELECT Round(CVErr(1),1/0)",
        "SELECT Round(IIf(True,CVErr(1),0),1/0)",
        "SELECT Sqr(IIf(True,CVErr(1),1/0))",
    ] {
        let parsed = parse_select(sql, Default::default()).unwrap();
        let Projection::Expression { expression, .. } = &parsed.query().projection[0] else {
            panic!("expression required")
        };
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, EvalLimits::default()).evaluate(expression),
            Err(EvalError::DivisionByZero)
        );
    }
}

#[test]
fn expression_math_shares_work_and_cancellation_before_random_mutation() {
    for (function, values) in [
        (MathFunction::Abs, vec![ExpressionValue::Empty]),
        (
            MathFunction::Rgb,
            vec![
                ExpressionValue::Empty,
                ExpressionValue::Empty,
                ExpressionValue::Empty,
            ],
        ),
        (
            MathFunction::Rnd,
            vec![ExpressionValue::Value(Value::Int16(1))],
        ),
    ] {
        let checks = Cell::new(0);
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        let state = RndState::default();
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_random(&state)
            .with_cancellation(&|| {
                checks.set(checks.get() + 1);
                false
            })
            .apply_expression_math(function, &values)
            .unwrap();
        for stop in 1..=checks.get() {
            let calls = Cell::new(0);
            let state = RndState::default();
            let mut budget = ResourceBudget::new(65536, 0, 10000);
            assert_eq!(
                Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                    .with_random(&state)
                    .with_cancellation(&|| {
                        calls.set(calls.get() + 1);
                        calls.get() >= stop
                    })
                    .apply_expression_math(function, &values),
                Err(EvalError::Cancelled)
            );
            assert_eq!(state.apply(None), RndState::default().apply(None));
        }
        let mut budget = ResourceBudget::new(65536, 0, 0);
        assert!(matches!(
            Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .apply_expression_math(function, &values),
            Err(EvalError::Limit(_))
        ));
    }
}

use super::*;
use crate::evaluation::{EvalLimits, NoBindings};
use std::cell::Cell;
use tablecraft_core::limits::ResourceBudget;

#[test]
fn radix_empty_is_zero_but_error_data_is_never_a_numeric_code() {
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    for function in [TextFunction::Hex, TextFunction::Oct] {
        assert_eq!(
            evaluator.apply_expression_radix(function, &ExpressionValue::Empty),
            Ok(Value::Text("0".into()))
        );
        assert_eq!(
            evaluator.apply_expression_radix(function, &ExpressionValue::Value(Value::Null)),
            Ok(Value::Null)
        );
        for code in [0, 1, 65535, u32::MAX] {
            assert_eq!(
                evaluator.apply_expression_radix(function, &ExpressionValue::Error(code)),
                Err(EvalError::TypeMismatch)
            );
        }
        assert_eq!(
            evaluator.apply_expression_radix(function, &ExpressionValue::Missing),
            Err(EvalError::NonScalarExpression)
        );
    }
    assert_eq!(
        evaluator.apply_expression_radix(TextFunction::Str, &ExpressionValue::Empty),
        Err(EvalError::UnsupportedExpression)
    );
}

#[test]
fn radix_empty_materialization_shares_cancellation_and_output_limits() {
    for function in [TextFunction::Hex, TextFunction::Oct] {
        let checks = Cell::new(0);
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_cancellation(&|| {
                checks.set(checks.get() + 1);
                false
            })
            .apply_expression_radix(function, &ExpressionValue::Empty)
            .unwrap();
        for stop in 1..=checks.get() {
            let calls = Cell::new(0);
            let mut budget = ResourceBudget::new(65536, 0, 10000);
            assert_eq!(
                Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                    .with_cancellation(&|| {
                        calls.set(calls.get() + 1);
                        calls.get() >= stop
                    })
                    .apply_expression_radix(function, &ExpressionValue::Empty),
                Err(EvalError::Cancelled)
            );
        }
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        assert_eq!(
            Evaluator::new(
                &NoBindings,
                &mut budget,
                EvalLimits {
                    max_text_bytes: 0,
                    ..Default::default()
                }
            )
            .apply_expression_radix(function, &ExpressionValue::Empty),
            Err(EvalError::TextLimit)
        );
    }
}

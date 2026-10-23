use super::*;
use crate::{
    evaluation::{EvalLimits, NoBindings},
    functions::conversions::text::NumberFormat,
};
use std::cell::Cell;
use tablecraft_core::limits::ResourceBudget;

#[test]
fn character_states_keep_numeric_empty_and_explicit_error_conversion_distinct() {
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    for function in [
        TextFunction::Chr,
        TextFunction::ChrString,
        TextFunction::ChrW,
    ] {
        for (source, expected) in [
            (ExpressionValue::Empty, Ok(Value::Text("\0".into()))),
            (ExpressionValue::Error(65), Err(EvalError::TypeMismatch)),
            (
                ExpressionValue::Missing,
                Err(EvalError::NonScalarExpression),
            ),
            (
                ExpressionValue::Value(Value::Null),
                Err(EvalError::InvalidNullConversion),
            ),
        ] {
            assert_eq!(
                evaluator.apply_expression_character(function, &source),
                expected
            );
        }
    }
    assert_eq!(
        evaluator.apply_expression_character(TextFunction::Asc, &ExpressionValue::Empty),
        Err(EvalError::UnsupportedExpression)
    );
}

#[test]
fn character_states_share_code_conversion_and_output_limits() {
    let source = ExpressionValue::Value(Value::Text("65.5".into()));
    let mut budget = ResourceBudget::new(5, 0, 1000);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_number_format(NumberFormat::EnUs)
            .apply_expression_character(TextFunction::ChrW, &source),
        Ok(Value::Text("B".into()))
    );

    let calls = Cell::new(0);
    let invoke = |stop| {
        calls.set(0);
        let cancelled = || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        };
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_number_format(NumberFormat::EnUs)
            .with_cancellation(&cancelled)
            .apply_expression_character(TextFunction::ChrW, &source)
    };
    assert_eq!(invoke(usize::MAX), Ok(Value::Text("B".into())));
    let total = calls.get();
    for stop in 1..=total {
        assert_eq!(invoke(stop), Err(EvalError::Cancelled));
    }
}

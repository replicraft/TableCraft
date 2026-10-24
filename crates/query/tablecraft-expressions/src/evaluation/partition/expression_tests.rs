use super::*;
use crate::{
    evaluation::{EvalLimits, NoBindings},
    functions::conversions::text::NumberFormat,
};
use std::cell::Cell;
use tablecraft_core::limits::ResourceBudget;

fn number(value: i32) -> ExpressionValue {
    ExpressionValue::Value(Value::Int32(value))
}

#[test]
fn partition_states_keep_null_gate_all_conversions_and_empty_numeric_targets() {
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    for (arguments, expected) in [
        (
            vec![
                ExpressionValue::Error(11),
                ExpressionValue::Value(Value::Null),
                number(9),
                number(2),
            ],
            Ok(Value::Null),
        ),
        (
            vec![number(1), number(-1), number(9), ExpressionValue::Error(11)],
            Err(EvalError::TypeMismatch),
        ),
        (
            vec![ExpressionValue::Missing, number(0), number(9), number(2)],
            Err(EvalError::NonScalarExpression),
        ),
        (
            vec![ExpressionValue::Empty, number(0), number(9), number(2)],
            Ok(Value::Text(" 0: 1".into())),
        ),
        (
            vec![number(3), ExpressionValue::Empty, number(9), number(2)],
            Ok(Value::Text(" 2: 3".into())),
        ),
        (
            vec![number(3), number(0), number(9), ExpressionValue::Empty],
            Err(EvalError::Domain),
        ),
    ] {
        assert_eq!(evaluator.apply_expression_partition(&arguments), expected);
    }
}

#[test]
fn partition_states_share_conversion_output_bytes_and_every_cancellation_checkpoint() {
    let arguments =
        ["2.5", "0", "9", "2"].map(|text| ExpressionValue::Value(Value::Text(text.into())));
    let mut budget = ResourceBudget::new(11, 0, 1000);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_number_format(NumberFormat::EnUs)
            .apply_expression_partition(&arguments),
        Ok(Value::Text(" 2: 3".into()))
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
            .apply_expression_partition(&arguments)
    };
    assert_eq!(invoke(usize::MAX), Ok(Value::Text(" 2: 3".into())));
    let total = calls.get();
    for stop in 1..=total {
        assert_eq!(invoke(stop), Err(EvalError::Cancelled));
    }
}

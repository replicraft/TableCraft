use super::*;
use crate::{
    evaluation::{EvalLimits, NoBindings},
    functions::conversions::text::NumberFormat,
};
use std::cell::Cell;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_sql::select::BinaryOperator as B;

#[test]
fn contextual_text_rounding_chooses_width_before_converting_operands() {
    let mut budget = ResourceBudget::new(4096, 0, 4096);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .with_number_format(NumberFormat::DeDe);
    assert_eq!(
        evaluator.apply_binary(B::And, &Value::Text("2,5".into()), &Value::Int16(3)),
        Ok(Value::Int32(2)),
    );
    assert_eq!(
        evaluator.apply_binary(
            B::Or,
            &Value::Text("9007199254740993".into()),
            &Value::Int64(0)
        ),
        Ok(Value::Int64(9007199254740993)),
    );
    assert_eq!(
        evaluator.logical_complement(&Value::Text("2,5".into())),
        Ok(Value::Int32(-3))
    );
    assert_eq!(
        evaluator.predicate_truth(&Value::Text("0,25".into())),
        Ok(Truth::True)
    );
    assert_eq!(
        evaluator.predicate_truth(&Value::Text("False".into())),
        Ok(Truth::False)
    );
    assert_eq!(
        evaluator.logical_complement(&Value::Text("False".into())),
        Err(EvalError::TypeMismatch)
    );
}

#[test]
fn missing_policy_limits_and_cancellation_are_operational_failures() {
    let text = Value::Text("2.5".into());
    let mut budget = ResourceBudget::new(4096, 0, 4096);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    assert_eq!(
        evaluator.logical_complement(&text),
        Err(EvalError::NumberFormatRequired)
    );
    assert_eq!(
        evaluator.predicate_truth(&text),
        Err(EvalError::NumberFormatRequired)
    );

    let mut budget = ResourceBudget::new(2, 0, 4096);
    let result = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .with_number_format(NumberFormat::EnUs)
        .logical_complement(&text);
    assert!(matches!(result, Err(EvalError::Limit(error)) if error.resource == Resource::Bytes));

    let mut budget = ResourceBudget::new(4096, 0, 4096);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_number_format(NumberFormat::EnUs)
            .with_cancellation(&|| true)
            .logical_complement(&Value::Text(String::new())),
        Err(EvalError::Cancelled),
    );
    let mut budget = ResourceBudget::new(4096, 0, 4096);
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
        .predicate_truth(&text),
        Err(EvalError::TextLimit),
    );
}

#[test]
fn both_text_operands_share_bytes_and_every_conversion_checkpoint_can_cancel() {
    let left = Value::Text("0".into());
    let right = Value::Text("2".into());
    let mut budget = ResourceBudget::new(1, 0, 4096);
    let result = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .with_number_format(NumberFormat::EnUs)
        .apply_binary(B::And, &left, &right);
    assert!(matches!(result, Err(EvalError::Limit(error)) if error.resource == Resource::Bytes));

    let checks = Cell::new(0);
    let observed = || {
        checks.set(checks.get() + 1);
        false
    };
    let mut budget = ResourceBudget::new(4096, 0, 4096);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_number_format(NumberFormat::EnUs)
            .with_cancellation(&observed)
            .apply_binary(B::And, &left, &right),
        Ok(Value::Int32(0)),
    );
    for stop in 1..=checks.get() {
        let current = Cell::new(0);
        let cancelled = || {
            current.set(current.get() + 1);
            current.get() == stop
        };
        let mut budget = ResourceBudget::new(4096, 0, 4096);
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_number_format(NumberFormat::EnUs)
                .with_cancellation(&cancelled)
                .apply_binary(B::And, &left, &right),
            Err(EvalError::Cancelled),
        );
    }
}

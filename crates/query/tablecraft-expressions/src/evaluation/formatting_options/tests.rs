use super::*;
use crate::{
    evaluation::{EvalLimits, NoBindings},
    functions::conversions::text::NumberFormat,
};
use std::cell::Cell;
use tablecraft_core::{limits::ResourceBudget, values::LegacyDateTime};

#[test]
fn numeric_format_options_validate_progressively_before_later_text_conversions() {
    let mut budget = ResourceBudget::new(1000, 0, 1000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .with_number_format(NumberFormat::EnUs);
    for (source, places, leading, expected) in [
        (
            Value::Int16(1),
            Value::Int16(256),
            Value::Text("bad".into()),
            EvalError::Domain,
        ),
        (
            Value::Null,
            Value::Int16(256),
            Value::Text("bad".into()),
            EvalError::InvalidNullConversion,
        ),
        (
            Value::Int16(1),
            Value::Int16(2),
            Value::Text("bad".into()),
            EvalError::TypeMismatch,
        ),
    ] {
        let arguments = [source, places, leading];
        assert_eq!(
            evaluator.apply_text_function(TextFunction::FormatNumber, &arguments),
            Err(expected)
        );
    }
}

#[test]
fn option_normalization_and_formatted_output_share_cumulative_budget() {
    let arguments = [
        Value::Text("1".into()),
        Value::Text("2.5".into()),
        Value::Text("-1".into()),
        Value::Text("0".into()),
        Value::Text("0".into()),
    ];
    let mut budget = ResourceBudget::new(100, 0, 1000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .with_number_format(NumberFormat::EnUs);
    assert_eq!(
        evaluator.apply_text_function(TextFunction::FormatNumber, &arguments),
        Ok(Value::Text("1.00".into()))
    );
    let mut budget = ResourceBudget::new(6, 0, 1000);
    assert!(matches!(
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_number_format(NumberFormat::EnUs)
            .apply_text_function(TextFunction::FormatNumber, &arguments),
        Err(EvalError::Limit(_))
    ));
}

#[test]
fn intrinsic_short_time_uses_number_policy_without_a_date_policy() {
    let arguments = [
        Value::DateTime(LegacyDateTime::from_ole_days(0.5).unwrap()),
        Value::Text("3.5".into()),
    ];
    let mut budget = ResourceBudget::new(8, 0, 1000);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_number_format(NumberFormat::EnUs)
            .apply_text_function(TextFunction::FormatDateTime, &arguments),
        Ok(Value::Text("12:00".into()))
    );
    let mut budget = ResourceBudget::new(1000, 0, 1000);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .apply_text_function(TextFunction::FormatDateTime, &arguments),
        Err(EvalError::NumberFormatRequired)
    );
}

#[test]
fn all_numeric_option_resolution_and_output_checkpoints_can_cancel() {
    let arguments = [
        Value::Int16(-123),
        Value::Text("2.5".into()),
        Value::Text("-1".into()),
        Value::Text("-1".into()),
        Value::Text("0".into()),
    ];
    let observed = Cell::new(0);
    let callback = || {
        observed.set(observed.get() + 1);
        false
    };
    let mut budget = ResourceBudget::new(1000, 0, 1000);
    Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .with_number_format(NumberFormat::EnUs)
        .with_cancellation(&callback)
        .apply_text_function(TextFunction::FormatNumber, &arguments)
        .unwrap();
    for stop in 1..=observed.get() {
        let current = Cell::new(0);
        let callback = || {
            current.set(current.get() + 1);
            current.get() == stop
        };
        let mut budget = ResourceBudget::new(1000, 0, 1000);
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_number_format(NumberFormat::EnUs)
                .with_cancellation(&callback)
                .apply_text_function(TextFunction::FormatNumber, &arguments),
            Err(EvalError::Cancelled)
        );
    }
}

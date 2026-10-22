use super::*;
use crate::{
    evaluation::{EvalLimits, NoBindings},
    functions::{conversions::text::NumberFormat, dates::parsing::DateTextFormat},
};
use std::cell::Cell;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{Double, ExtendedDateTime},
};

#[test]
fn numeric_and_text_sources_share_cdate_with_existing_component_precision() {
    let mut budget = ResourceBudget::new(1000, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .with_date_format(DateTextFormat::Iso)
        .with_number_format(NumberFormat::DeDe);
    for (value, components) in [
        (
            Value::Double(Double::from_float(-1.25)),
            [1899, 12, 29, 6, 0, 0],
        ),
        (Value::Text("45658,5".into()), [2025, 1, 1, 12, 0, 0]),
        (
            Value::Text("2024-02-29 23:59:59".into()),
            [2024, 2, 29, 23, 59, 59],
        ),
        (Value::Boolean(true), [1899, 12, 29, 0, 0, 0]),
    ] {
        for (function, expected) in [
            DateFunction::Year,
            DateFunction::Month,
            DateFunction::Day,
            DateFunction::Hour,
            DateFunction::Minute,
            DateFunction::Second,
        ]
        .into_iter()
        .zip(components)
        {
            assert_eq!(
                evaluator.apply_date_function(function, &value),
                Ok(Value::Int16(expected))
            );
        }
    }
}

#[test]
fn null_and_extended_sources_retain_their_carrier_policy() {
    let mut budget = ResourceBudget::new(0, 0, 1000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    assert_eq!(
        evaluator.apply_date_function(DateFunction::Year, &Value::Null),
        Ok(Value::Null)
    );
    let extended =
        Value::DateTimeExtended(ExtendedDateTime::new(2024, 2, 29, 23, 59, 59, 9_999_999).unwrap());
    assert_eq!(
        evaluator.apply_date_function(DateFunction::Day, &extended),
        Ok(Value::Int16(29))
    );
    assert_eq!(
        evaluator.apply_date_function(DateFunction::Second, &extended),
        Ok(Value::Int16(59))
    );
    assert_eq!(
        evaluator.apply_date_function(DateFunction::Year, &Value::Text("2025-01-01".into())),
        Err(EvalError::DateFormatRequired)
    );
    assert_eq!(
        evaluator.apply_date_function(DateFunction::Year, &Value::Int32(2_958_466)),
        Err(EvalError::Overflow)
    );
}

#[test]
fn text_source_size_and_normalization_obey_cumulative_limits() {
    let source = Value::Text("45658.5".into());
    for (bytes, expected) in [(7, None), (6, Some(Resource::Bytes))] {
        let mut budget = ResourceBudget::new(bytes, 0, 10000);
        let result = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_date_format(DateTextFormat::Iso)
            .with_number_format(NumberFormat::EnUs)
            .apply_date_function(DateFunction::Hour, &source);
        if let Some(resource) = expected {
            assert!(matches!(result, Err(EvalError::Limit(error)) if error.resource == resource));
        } else {
            assert_eq!(result, Ok(Value::Int16(12)));
        }
    }
    let mut budget = ResourceBudget::new(100, 0, 1000);
    let result = Evaluator::new(
        &NoBindings,
        &mut budget,
        EvalLimits {
            max_text_bytes: 6,
            ..Default::default()
        },
    )
    .with_date_format(DateTextFormat::Iso)
    .with_number_format(NumberFormat::EnUs)
    .apply_date_function(DateFunction::Year, &source);
    assert_eq!(result, Err(EvalError::TextLimit));
}

#[test]
fn every_observed_component_conversion_checkpoint_can_cancel() {
    let calls = Cell::new(0);
    let run = |stop| {
        let cancelled = || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        };
        let mut budget = ResourceBudget::new(1000, 0, 10000);
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_date_format(DateTextFormat::Iso)
            .with_number_format(NumberFormat::EnUs)
            .with_cancellation(&cancelled)
            .apply_date_function(DateFunction::Hour, &Value::Text("45658.5".into()))
    };
    assert_eq!(run(usize::MAX), Ok(Value::Int16(12)));
    let count = calls.get();
    for stop in 1..=count {
        calls.set(0);
        assert_eq!(run(stop), Err(EvalError::Cancelled));
    }
}

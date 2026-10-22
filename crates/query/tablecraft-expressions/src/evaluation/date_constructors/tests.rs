use super::*;
use crate::{
    evaluation::{EvalLimits, NoBindings},
    functions::conversions::text::NumberFormat,
};
use std::cell::Cell;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{Double, LegacyDateTime},
};

fn text(value: &str) -> Value {
    Value::Text(value.into())
}

#[test]
fn contextual_components_use_integer_rounding_without_a_date_syntax_policy() {
    let mut budget = ResourceBudget::new(1000, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .with_number_format(NumberFormat::DeDe);
    let actual = evaluator
        .apply_date_constructor(
            DateConstructor::DateSerial,
            &[text("2024,5"), text("2,5"), text("28,5")],
        )
        .unwrap();
    let expected = Value::DateTime(
        LegacyDateTime::from_calendar("2024-02-28T00:00:00".parse().unwrap()).unwrap(),
    );
    assert_eq!(actual, expected);
    let actual = evaluator
        .apply_date_constructor(
            DateConstructor::TimeSerial,
            &[text("-0,5"), text("1,5"), text("0")],
        )
        .unwrap();
    let expected = Value::DateTime(LegacyDateTime::from_ole_days(120.0 / 86400.0).unwrap());
    assert_eq!(actual, expected);
}

#[test]
fn component_validation_is_progressive_before_calendar_normalization() {
    let mut budget = ResourceBudget::new(100, 0, 1000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    for (arguments, expected) in [
        (
            [
                Value::Double(Double::from_float(32767.5)),
                text("bad"),
                Value::Null,
            ],
            EvalError::Overflow,
        ),
        (
            [Value::Null, text("bad"), Value::Null],
            EvalError::InvalidNullConversion,
        ),
        (
            [text("bad"), Value::Null, Value::Null],
            EvalError::NumberFormatRequired,
        ),
    ] {
        assert_eq!(
            evaluator.apply_date_constructor(DateConstructor::DateSerial, &arguments),
            Err(expected)
        );
    }
    assert_eq!(
        evaluator.apply_date_constructor(DateConstructor::DateSerial, &[text("bad")]),
        Err(EvalError::UnsupportedExpression)
    );
}

#[test]
fn all_component_normalizations_share_exact_cumulative_limits() {
    let arguments = [text("2024"), text("2.5"), text("28.5")];
    for (bytes, expected) in [(11, None), (10, Some(Resource::Bytes))] {
        let mut budget = ResourceBudget::new(bytes, 0, 10000);
        let result = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_number_format(NumberFormat::EnUs)
            .apply_date_constructor(DateConstructor::DateSerial, &arguments);
        if let Some(resource) = expected {
            assert!(matches!(result, Err(EvalError::Limit(error)) if error.resource == resource));
        } else {
            assert!(result.is_ok());
        }
    }
    let mut budget = ResourceBudget::new(100, 0, 1000);
    let result = Evaluator::new(
        &NoBindings,
        &mut budget,
        EvalLimits {
            max_text_bytes: 3,
            ..Default::default()
        },
    )
    .with_number_format(NumberFormat::EnUs)
    .apply_date_constructor(DateConstructor::DateSerial, &arguments);
    assert_eq!(result, Err(EvalError::TextLimit));
}

#[test]
fn every_observed_constructor_conversion_checkpoint_can_cancel() {
    for function in [DateConstructor::DateSerial, DateConstructor::TimeSerial] {
        let calls = Cell::new(0);
        let run = |stop| {
            let cancelled = || {
                calls.set(calls.get() + 1);
                calls.get() == stop
            };
            let mut budget = ResourceBudget::new(1000, 0, 10000);
            Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_number_format(NumberFormat::EnUs)
                .with_cancellation(&cancelled)
                .apply_date_constructor(function, &[text("2024"), text("2.5"), text("28.5")])
        };
        assert!(run(usize::MAX).is_ok());
        let count = calls.get();
        for stop in 1..=count {
            calls.set(0);
            assert_eq!(run(stop), Err(EvalError::Cancelled));
        }
    }
}

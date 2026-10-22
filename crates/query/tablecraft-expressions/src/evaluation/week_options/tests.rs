use super::*;
use crate::{
    evaluation::{EvalLimits, NoBindings},
    functions::{conversions::text::NumberFormat, text::TextFunction},
};
use std::cell::Cell;
use tablecraft_core::limits::{Resource, ResourceBudget};

fn date() -> Value {
    Value::DateTime(
        tablecraft_core::values::LegacyDateTime::from_calendar(
            "2025-01-01T00:00:00".parse().unwrap(),
        )
        .unwrap(),
    )
}

#[test]
fn option_conversion_is_progressive_and_retains_function_error_order() {
    let mut budget = ResourceBudget::new(100, 0, 1000);
    let mut eval = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .with_number_format(NumberFormat::EnUs);
    let bad = Value::Text("bad".into());
    assert_eq!(
        eval.apply_date_part(&bad, &date(), Some(&bad), None),
        Err(EvalError::InvalidDateInterval)
    );
    assert_eq!(
        eval.apply_date_part(
            &Value::Text("ww".into()),
            &Value::Null,
            Some(&Value::Text("8".into())),
            Some(&bad),
        ),
        Err(EvalError::Domain)
    );
    assert_eq!(
        eval.apply_text_function(
            TextFunction::Format,
            &[Value::Null, bad.clone(), bad, Value::Null],
        ),
        Err(EvalError::InvalidNullConversion)
    );
    assert_eq!(
        eval.apply_date_part(
            &Value::Text("ww".into()),
            &Value::Null,
            Some(&Value::Null),
            None,
        ),
        Err(EvalError::TypeMismatch)
    );
}

#[test]
fn normalization_is_cumulative_and_date_payloads_do_not_allocate() {
    let invoke = |bytes| {
        let mut budget = ResourceBudget::new(bytes, 0, 1000);
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_number_format(NumberFormat::EnUs)
            .apply_date_part(
                &Value::Text("ww".into()),
                &date(),
                Some(&Value::Text("2.5".into())),
                Some(&Value::Text("1.5".into())),
            )
    };
    assert_eq!(invoke(6), Ok(Value::Int16(1)));
    assert!(matches!(invoke(5), Err(EvalError::Limit(error)) if error.resource == Resource::Bytes));

    let mut budget = ResourceBudget::new(100, 0, 1000);
    let result = Evaluator::new(
        &NoBindings,
        &mut budget,
        EvalLimits {
            max_text_bytes: 2,
            ..Default::default()
        },
    )
    .with_number_format(NumberFormat::EnUs)
    .apply_weekday(&date(), Some(&Value::Text("2.5".into())));
    assert_eq!(result, Err(EvalError::TextLimit));
}

#[test]
fn every_observed_week_option_checkpoint_obeys_cancellation() {
    for target in 0..4 {
        let run = |stop, calls: &Cell<usize>| {
            let cancelled = || {
                calls.set(calls.get() + 1);
                calls.get() == stop
            };
            let mut budget = ResourceBudget::new(1000, 0, 10000);
            let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_number_format(NumberFormat::EnUs)
                .with_cancellation(&cancelled);
            if target == 0 {
                evaluator.apply_text_function(
                    TextFunction::Format,
                    &[
                        date(),
                        Value::Text("w ww".into()),
                        Value::Text("2.5".into()),
                        Value::Text("1.5".into()),
                    ],
                )
            } else if target == 1 {
                evaluator.apply_date_part(
                    &Value::Text("ww".into()),
                    &date(),
                    Some(&Value::Text("2.5".into())),
                    Some(&Value::Text("1.5".into())),
                )
            } else if target == 2 {
                evaluator.apply_date_diff(
                    &Value::Text("ww".into()),
                    &date(),
                    &date(),
                    Some(&Value::Text("2.5".into())),
                    Some(&Value::Text("1.5".into())),
                )
            } else {
                evaluator.apply_weekday(&date(), Some(&Value::Text("2.5".into())))
            }
        };
        let calls = Cell::new(0);
        assert!(run(usize::MAX, &calls).is_ok());
        let total = calls.get();
        for stop in 1..=total {
            calls.set(0);
            assert_eq!(run(stop, &calls), Err(EvalError::Cancelled), "stop {stop}");
        }
    }
}

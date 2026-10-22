use super::*;
use crate::{
    evaluation::{EvalLimits, NoBindings},
    functions::{conversions::text::NumberFormat, dates::parsing::DateTextFormat},
};
use std::cell::Cell;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{ExtendedDateTime, LegacyDateTime},
};

fn text(value: &str) -> Value {
    Value::Text(value.into())
}

fn date(value: &str) -> Value {
    Value::DateTime(LegacyDateTime::from_calendar(value.parse().unwrap()).unwrap())
}

#[test]
fn coerced_sources_and_counts_retain_calendar_arithmetic_and_carriers() {
    let mut budget = ResourceBudget::new(1000, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .with_date_format(DateTextFormat::Iso)
        .with_number_format(NumberFormat::DeDe);
    assert_eq!(
        evaluator.apply_date_add(&text("m"), &text("0,5"), &text("2024-01-31 12:00:00")),
        Ok(date("2024-01-31T12:00:00"))
    );
    assert_eq!(
        evaluator.apply_date_add(&text("m"), &text("1,5"), &text("2024-01-31 12:00:00")),
        Ok(date("2024-03-31T12:00:00"))
    );
    assert_eq!(
        evaluator.apply_date_part(&text("h"), &text("-1,25"), None, None),
        Ok(Value::Int16(6))
    );
    assert_eq!(
        evaluator.apply_date_diff(&text("h"), &text("-1,25"), &Value::Int16(0), None, None),
        Ok(Value::Int32(18))
    );
    let extended = Value::DateTimeExtended(ExtendedDateTime::new(1, 1, 1, 0, 0, 0, 1).unwrap());
    assert_eq!(
        evaluator.apply_date_add(&text("s"), &text("0"), &extended),
        Ok(extended.clone())
    );
    assert_eq!(
        evaluator.apply_date_part(&text("yyyy"), &extended, None, None),
        Ok(Value::Int16(1))
    );
    assert_eq!(
        evaluator.apply_date_diff(&text("s"), &extended, &extended, None, None),
        Ok(Value::Int32(0))
    );
}

#[test]
fn controls_validate_before_date_conversion_or_null_propagation() {
    let mut budget = ResourceBudget::new(1000, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    assert_eq!(
        evaluator.apply_date_add(&text("bad"), &text("bad"), &text("bad")),
        Err(EvalError::InvalidDateInterval)
    );
    assert_eq!(
        evaluator.apply_date_add(&text("d"), &text("bad"), &Value::Null),
        Err(EvalError::NumberFormatRequired)
    );
    assert_eq!(
        evaluator.apply_date_add(&text("d"), &Value::Null, &Value::Null),
        Err(EvalError::TypeMismatch)
    );
    assert_eq!(
        evaluator.apply_date_part(&text("bad"), &text("bad"), None, None),
        Err(EvalError::InvalidDateInterval)
    );
    assert_eq!(
        evaluator.apply_date_part(&text("d"), &text("bad"), Some(&Value::Int16(8)), None),
        Err(EvalError::Domain)
    );
    assert_eq!(
        evaluator.apply_date_diff(
            &text("w"),
            &text("bad"),
            &text("bad"),
            Some(&Value::Int16(2)),
            None
        ),
        Err(EvalError::UnsupportedExpression)
    );
    assert_eq!(
        evaluator.apply_date_add(&text("d"), &Value::Int16(1), &Value::Null),
        Ok(Value::Null)
    );
    assert_eq!(
        evaluator.apply_date_part(&text("d"), &Value::Null, None, None),
        Ok(Value::Null)
    );
}

#[test]
fn both_diff_sources_validate_before_null_but_existing_dates_are_not_narrowed() {
    let mut budget = ResourceBudget::new(1000, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .with_date_format(DateTextFormat::Iso);
    for (first, second) in [(Value::Null, text("bad")), (text("bad"), Value::Null)] {
        assert_eq!(
            evaluator.apply_date_diff(&text("d"), &first, &second, None, None),
            Err(EvalError::TypeMismatch)
        );
    }
    assert_eq!(
        evaluator.apply_date_diff(&text("d"), &Value::Null, &text("2025-01-01"), None, None),
        Ok(Value::Null)
    );
    assert_eq!(
        evaluator.apply_date_diff(
            &text("d"),
            &Value::Null,
            &Value::Int32(2_958_466),
            None,
            None
        ),
        Err(EvalError::Overflow)
    );
    let extended = Value::DateTimeExtended(ExtendedDateTime::new(1, 1, 1, 0, 0, 0, 1).unwrap());
    assert_eq!(
        evaluator.apply_date_diff(&text("s"), &Value::Null, &extended, None, None),
        Ok(Value::Null)
    );
}

#[test]
fn controls_and_both_source_normalizations_share_allocation_limits() {
    for (target, exact) in [(0, 7), (1, 10)] {
        let run = |bytes| {
            let mut budget = ResourceBudget::new(bytes, 0, 10000);
            let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_date_format(DateTextFormat::Iso)
                .with_number_format(NumberFormat::EnUs);
            if target == 0 {
                evaluator.apply_date_add(&text("d"), &text("2.5"), &text("0.25"))
            } else {
                evaluator.apply_date_diff(
                    &text("d"),
                    &text("0.25"),
                    &text("1.25"),
                    Some(&text("2")),
                    Some(&text("2")),
                )
            }
        };
        assert!(run(exact).is_ok());
        assert!(
            matches!(run(exact - 1), Err(EvalError::Limit(error)) if error.resource == Resource::Bytes)
        );
    }
}

#[test]
fn every_observed_control_and_source_conversion_checkpoint_can_cancel() {
    for target in 0..3 {
        let calls = Cell::new(0);
        let run = |stop| {
            let cancelled = || {
                calls.set(calls.get() + 1);
                calls.get() == stop
            };
            let mut budget = ResourceBudget::new(1000, 0, 10000);
            let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_date_format(DateTextFormat::Iso)
                .with_number_format(NumberFormat::EnUs)
                .with_cancellation(&cancelled);
            match target {
                0 => evaluator.apply_date_add(&text("d"), &text("2.5"), &text("0.25")),
                1 => evaluator.apply_date_part(
                    &text("ww"),
                    &text("1.25"),
                    Some(&text("2")),
                    Some(&text("2")),
                ),
                _ => evaluator.apply_date_diff(
                    &text("d"),
                    &text("0.25"),
                    &text("1.25"),
                    Some(&text("2")),
                    Some(&text("2")),
                ),
            }
        };
        assert!(run(usize::MAX).is_ok());
        let total = calls.get();
        for stop in 1..=total {
            calls.set(0);
            assert_eq!(
                run(stop),
                Err(EvalError::Cancelled),
                "target {target}, stop {stop}"
            );
        }
    }
}

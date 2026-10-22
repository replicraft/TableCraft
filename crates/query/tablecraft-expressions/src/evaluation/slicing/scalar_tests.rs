use super::*;
use crate::{
    evaluation::{EvalLimits, NoBindings},
    functions::{conversions::text::NumberFormat, text::cstr::DateTextFormat},
};
use std::cell::Cell;
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Currency, LegacyDateTime},
};

#[test]
fn scalar_sources_use_explicit_formats_before_utf16_slicing() {
    for format in [NumberFormat::EnUs, NumberFormat::DeDe] {
        let fractional = if format == NumberFormat::EnUs {
            ".5"
        } else {
            ",5"
        };
        for (function, source, position, length, expected) in [
            (TextFunction::Left, Value::Int64(12345), 2, None, "12"),
            (TextFunction::Right, Value::Boolean(true), 2, None, "ue"),
            (TextFunction::Mid, Value::Int64(12345), 2, Some(2), "23"),
            (
                TextFunction::Right,
                Value::Currency(Currency::from_scaled(125_000)),
                2,
                None,
                fractional,
            ),
            (
                TextFunction::Left,
                Value::DateTime(LegacyDateTime::from_ole_days(2.0).unwrap()),
                4,
                None,
                "1900",
            ),
        ] {
            let mut arguments = vec![source, Value::Int32(position)];
            if let Some(length) = length {
                arguments.push(Value::Int32(length));
            }
            let mut budget = ResourceBudget::new(1000, 0, 1000);
            assert_eq!(
                Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                    .with_number_format(format)
                    .with_date_format(DateTextFormat::Iso)
                    .apply_text_function(function, &arguments),
                Ok(Value::Text(expected.into()))
            );
        }
    }
}

#[test]
fn null_source_shortcut_and_string_or_position_errors_remain_distinct() {
    let mut budget = ResourceBudget::new(1000, 0, 1000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    for function in [TextFunction::Left, TextFunction::Right, TextFunction::Mid] {
        assert_eq!(
            evaluator.apply_text_function(function, &[Value::Null, Value::Binary(vec![1])]),
            Ok(Value::Null)
        );
        assert_eq!(
            evaluator.apply_text_function(function, &[Value::Int64(123), Value::Int16(0)]),
            Err(EvalError::NumberFormatRequired)
        );
        assert_eq!(
            evaluator.apply_text_function(function, &[Value::Binary(vec![1]), Value::Int16(1)]),
            Err(EvalError::TypeMismatch)
        );
    }
    assert_eq!(
        evaluator.apply_text_function(
            TextFunction::Left,
            &[Value::Text("a🦀b".into()), Value::Int16(2)]
        ),
        Err(EvalError::UnrepresentableText)
    );
    assert_eq!(
        evaluator.apply_text_function(
            TextFunction::Mid,
            &[Value::Text("a🦀b".into()), Value::Int16(2), Value::Int16(2)]
        ),
        Ok(Value::Text("🦀".into()))
    );
}

#[test]
fn source_formatting_and_slice_materialization_share_budgets_and_cancellation() {
    let arguments = [Value::Int64(12345), Value::Int16(2)];
    for (allowance, succeeds) in [(6, false), (7, true)] {
        let mut budget = ResourceBudget::new(allowance, 0, 1000);
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_number_format(NumberFormat::EnUs)
                .apply_text_function(TextFunction::Left, &arguments)
                .is_ok(),
            succeeds
        );
    }
    let calls = Cell::new(0);
    let invoke = |stop| {
        calls.set(0);
        let cancelled = || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        };
        let mut budget = ResourceBudget::new(1000, 0, 1000);
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_number_format(NumberFormat::EnUs)
            .with_cancellation(&cancelled)
            .apply_text_function(TextFunction::Left, &arguments)
    };
    assert_eq!(invoke(usize::MAX), Ok(Value::Text("12".into())));
    let observed = calls.get();
    for stop in 1..=observed {
        assert_eq!(invoke(stop), Err(EvalError::Cancelled));
    }
}

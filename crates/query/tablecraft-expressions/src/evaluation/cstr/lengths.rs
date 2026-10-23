use super::*;
use crate::{
    evaluation::{EvalLimits, NoBindings},
    functions::{conversions::text::NumberFormat, text::cstr::DateTextFormat},
};
use std::cell::Cell;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{Currency, Decimal, Double, LegacyDateTime, Single},
};

#[test]
fn query_lengths_measure_scalar_spellings_in_utf16_instead_of_storage_widths() {
    for format in [NumberFormat::EnUs, NumberFormat::DeDe] {
        for (value, units) in [
            (Value::Boolean(true), 4),
            (Value::Boolean(false), 5),
            (Value::Byte(255), 3),
            (Value::Int16(-32768), 6),
            (Value::Int32(12345), 5),
            (Value::Int64(i64::MIN), 20),
            (Value::Single(Single::from_float(12.5)), 4),
            (Value::Double(Double::from_float(-12.5)), 5),
            (Value::Currency(Currency::from_scaled(125_000)), 4),
            (Value::Decimal(Decimal::new(125, 1).unwrap()), 4),
            (
                Value::DateTime(LegacyDateTime::from_ole_days(2.0).unwrap()),
                10,
            ),
            (Value::Text("é中🦀\0".into()), 5),
            (Value::Text(String::new()), 0),
        ] {
            for (function, factor) in [(TextFunction::Len, 1), (TextFunction::LenB, 2)] {
                let mut budget = ResourceBudget::new(1000, 0, 10_000);
                assert_eq!(
                    Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                        .with_number_format(format)
                        .with_date_format(DateTextFormat::Iso)
                        .apply_text_function(function, std::slice::from_ref(&value)),
                    Ok(Value::Int32(units * factor)),
                    "{function:?} {value:?} {format:?}"
                );
            }
        }
    }
}

#[test]
fn query_lengths_require_policies_and_keep_null_and_binary_distinct() {
    for function in [TextFunction::Len, TextFunction::LenB] {
        let mut budget = ResourceBudget::new(1000, 0, 1000);
        let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
        assert_eq!(
            evaluator.apply_text_function(function, &[Value::Null]),
            Ok(Value::Null)
        );
        assert_eq!(
            evaluator.apply_text_function(function, &[Value::Int32(12345)]),
            Err(EvalError::NumberFormatRequired)
        );
        assert_eq!(
            evaluator.apply_text_function(
                function,
                &[Value::DateTime(LegacyDateTime::from_ole_days(2.0).unwrap())]
            ),
            Err(EvalError::DateFormatRequired)
        );
        assert_eq!(
            evaluator.apply_text_function(function, &[Value::Binary(vec![65])]),
            Err(EvalError::TypeMismatch)
        );
        assert_eq!(
            evaluator.apply_text_function(function, &[Value::Int32(1), Value::Int32(2)]),
            Err(EvalError::UnsupportedExpression)
        );
    }
}

#[test]
fn scalar_length_conversion_and_scan_share_limits_and_cancellation() {
    for function in [TextFunction::Len, TextFunction::LenB] {
        let calls = Cell::new(0);
        let invoke = |stop, bytes, steps, text_bytes| {
            calls.set(0);
            let cancelled = || {
                calls.set(calls.get() + 1);
                calls.get() == stop
            };
            let mut budget = ResourceBudget::new(bytes, 0, steps);
            Evaluator::new(
                &NoBindings,
                &mut budget,
                EvalLimits {
                    max_text_bytes: text_bytes,
                    ..Default::default()
                },
            )
            .with_number_format(NumberFormat::EnUs)
            .with_cancellation(&cancelled)
            .apply_text_function(function, &[Value::Int64(i64::MIN)])
        };
        assert!(invoke(usize::MAX, 1000, 1000, 100).is_ok());
        let observed = calls.get();
        assert!(observed > 1);
        for stop in 1..=observed {
            assert_eq!(invoke(stop, 1000, 1000, 100), Err(EvalError::Cancelled));
        }
        assert!(
            matches!(invoke(usize::MAX, 1, 1000, 100), Err(EvalError::Limit(error)) if error.resource == Resource::Bytes)
        );
        assert!(
            matches!(invoke(usize::MAX, 1000, 1, 100), Err(EvalError::Limit(error)) if error.resource == Resource::Steps)
        );
        assert_eq!(
            invoke(usize::MAX, 1000, 1000, 19),
            Err(EvalError::TextLimit)
        );
    }
}

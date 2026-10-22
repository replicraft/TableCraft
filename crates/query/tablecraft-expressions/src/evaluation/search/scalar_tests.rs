use super::*;
use crate::{
    evaluation::{EvalLimits, NoBindings},
    functions::{conversions::text::NumberFormat, text::cstr::DateTextFormat},
};
use std::cell::Cell;
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Currency, Decimal, LegacyDateTime},
};

#[test]
fn scalar_spellings_reuse_explicit_formats_and_keep_text_borrowed() {
    for format in [NumberFormat::EnUs, NumberFormat::DeDe] {
        let decimal_separator = if format == NumberFormat::EnUs {
            ".5"
        } else {
            ",5"
        };
        let cases = [
            (Value::Int64(123123), Value::Int16(23), 2, 5),
            (Value::Boolean(true), Value::Text("ru".into()), 2, 2),
            (
                Value::Currency(Currency::from_scaled(125_000)),
                Value::Text(decimal_separator.into()),
                3,
                3,
            ),
            (
                Value::Decimal(Decimal::new(125, 1).unwrap()),
                Value::Text(decimal_separator.into()),
                3,
                3,
            ),
            (
                Value::DateTime(LegacyDateTime::from_ole_days(2.0).unwrap()),
                Value::Text("01".into()),
                6,
                9,
            ),
        ];
        for (haystack, needle, forward, reverse) in cases {
            for function in [TextFunction::InStr, TextFunction::InStrRev] {
                let arguments = if function == TextFunction::InStr {
                    vec![
                        Value::Int16(1),
                        haystack.clone(),
                        needle.clone(),
                        Value::Int16(0),
                    ]
                } else {
                    vec![haystack.clone(), needle.clone()]
                };
                let mut budget = ResourceBudget::new(1000, 0, 10_000);
                let actual = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                    .with_number_format(format)
                    .with_date_format(DateTextFormat::Iso)
                    .apply_text_function(function, &arguments);
                assert_eq!(
                    actual,
                    Ok(Value::Int32(if function == TextFunction::InStr {
                        forward
                    } else {
                        reverse
                    }))
                );
            }
        }
    }
    let mut budget = ResourceBudget::new(0, 0, 1000);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default()).apply_text_function(
            TextFunction::InStrRev,
            &[Value::Text("banana".into()), Value::Text("an".into())]
        ),
        Ok(Value::Int32(4))
    );
}

#[test]
fn mode_start_null_and_missing_format_boundaries_remain_explicit() {
    let mut budget = ResourceBudget::new(1000, 0, 1000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    for (function, arguments, expected) in [
        (
            TextFunction::InStrRev,
            vec![Value::Int64(123), Value::Null],
            Ok(Value::Null),
        ),
        (
            TextFunction::InStr,
            vec![
                Value::Int16(0),
                Value::Int64(123),
                Value::Null,
                Value::Int16(0),
            ],
            Err(EvalError::Domain),
        ),
        (
            TextFunction::InStrRev,
            vec![
                Value::Int64(123),
                Value::Null,
                Value::Int16(1),
                Value::Int16(3),
            ],
            Err(EvalError::Domain),
        ),
        (
            TextFunction::InStrRev,
            vec![Value::Int64(123), Value::Text("2".into())],
            Err(EvalError::NumberFormatRequired),
        ),
        (
            TextFunction::InStrRev,
            vec![Value::Binary(vec![1]), Value::Null],
            Ok(Value::Null),
        ),
        (
            TextFunction::InStrRev,
            vec![Value::Binary(vec![1]), Value::Text("2".into())],
            Err(EvalError::TypeMismatch),
        ),
    ] {
        assert_eq!(
            evaluator.apply_text_function(function, &arguments),
            expected
        );
    }
}

#[test]
fn scalar_conversion_respects_text_bytes_budgets_and_each_cancellation_checkpoint() {
    let arguments = [Value::Int64(123123), Value::Int16(23)];
    let mut budget = ResourceBudget::new(5, 0, 1000);
    assert!(matches!(
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_number_format(NumberFormat::EnUs)
            .apply_text_function(TextFunction::InStrRev, &arguments),
        Err(EvalError::Limit(_))
    ));
    let mut budget = ResourceBudget::new(1000, 0, 1000);
    assert_eq!(
        Evaluator::new(
            &NoBindings,
            &mut budget,
            EvalLimits {
                max_text_bytes: 5,
                ..Default::default()
            }
        )
        .with_number_format(NumberFormat::EnUs)
        .apply_text_function(TextFunction::InStrRev, &arguments),
        Err(EvalError::TextLimit)
    );
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
            .apply_text_function(TextFunction::InStrRev, &arguments)
    };
    assert_eq!(invoke(usize::MAX), Ok(Value::Int32(5)));
    let observed = calls.get();
    for stop in 1..=observed {
        assert_eq!(invoke(stop), Err(EvalError::Cancelled));
    }
}

use super::*;
use crate::{
    evaluation::{EvalLimits, NoBindings},
    functions::{
        conversions::text::NumberFormat,
        text::{casing::CaseLocale, cstr::DateTextFormat},
    },
};
use std::cell::Cell;
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Currency, Double, LegacyDateTime},
};

#[test]
fn scalar_trimming_and_casing_reuse_explicit_format_and_language_policies() {
    let locale: CaseLocale = "en-US".parse().unwrap();
    for format in [NumberFormat::EnUs, NumberFormat::DeDe] {
        let fraction = if format == NumberFormat::EnUs {
            "12.5"
        } else {
            "12,5"
        };
        for (value, spelling) in [
            (Value::Int64(-12345), "-12345"),
            (Value::Currency(Currency::from_scaled(125_000)), fraction),
            (Value::Boolean(true), "True"),
            (
                Value::DateTime(LegacyDateTime::from_ole_days(2.0).unwrap()),
                "1900-01-01",
            ),
        ] {
            for function in [
                TextFunction::LTrim,
                TextFunction::RTrim,
                TextFunction::Trim,
                TextFunction::LCase,
                TextFunction::UCase,
            ] {
                let expected = match function {
                    TextFunction::LCase => spelling.to_ascii_lowercase(),
                    TextFunction::UCase => spelling.to_ascii_uppercase(),
                    _ => spelling.into(),
                };
                let mut budget = ResourceBudget::new(1000, 0, 10_000);
                assert_eq!(
                    Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                        .with_number_format(format)
                        .with_date_format(DateTextFormat::Iso)
                        .with_case_locale(&locale)
                        .apply_text_function(function, std::slice::from_ref(&value)),
                    Ok(Value::Text(expected))
                );
            }
        }
    }
}

#[test]
fn null_and_missing_format_or_case_policies_remain_distinct() {
    let mut budget = ResourceBudget::new(1000, 0, 1000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    for function in [
        TextFunction::LTrim,
        TextFunction::RTrim,
        TextFunction::Trim,
        TextFunction::LCase,
        TextFunction::UCase,
    ] {
        assert_eq!(
            evaluator.apply_text_function(function, &[Value::Null]),
            Ok(Value::Null)
        );
        assert_eq!(
            evaluator.apply_text_function(function, &[Value::Int64(12)]),
            Err(EvalError::NumberFormatRequired)
        );
        assert_eq!(
            evaluator.apply_text_function(function, &[Value::Binary(vec![1])]),
            Err(EvalError::TypeMismatch)
        );
    }
    assert_eq!(
        evaluator.apply_text_function(TextFunction::LCase, &[Value::Boolean(true)]),
        Err(EvalError::CaseLocaleRequired)
    );
    assert_eq!(
        evaluator.apply_text_function(
            TextFunction::Trim,
            &[Value::Text(" \t\u{a0}x\u{a0}\t ".into())]
        ),
        Ok(Value::Text("\t\u{a0}x\u{a0}\t".into()))
    );
}

#[test]
fn conversion_and_output_share_budgets_with_borrowed_text_and_cancellation() {
    for (value, allowance, succeeds) in [
        (Value::Int64(12345), 9, false),
        (Value::Int64(12345), 10, true),
        (Value::Text("12345".into()), 5, true),
    ] {
        let mut budget = ResourceBudget::new(allowance, 0, 1000);
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_number_format(NumberFormat::EnUs)
                .apply_text_function(TextFunction::Trim, &[value])
                .is_ok(),
            succeeds
        );
    }
    let locale: CaseLocale = "en-US".parse().unwrap();
    for function in [TextFunction::Trim, TextFunction::UCase] {
        let calls = Cell::new(0);
        let invoke = |stop| {
            calls.set(0);
            let cancelled = || {
                calls.set(calls.get() + 1);
                calls.get() == stop
            };
            let mut budget = ResourceBudget::new(1000, 0, 10_000);
            Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_number_format(NumberFormat::EnUs)
                .with_case_locale(&locale)
                .with_cancellation(&cancelled)
                .apply_text_function(function, &[Value::Int64(12345)])
        };
        assert_eq!(invoke(usize::MAX), Ok(Value::Text("12345".into())));
        let observed = calls.get();
        for stop in 1..=observed {
            assert_eq!(invoke(stop), Err(EvalError::Cancelled));
        }
    }
}

#[test]
fn character_codes_and_reversal_share_scalar_spelling_without_numeric_code_casts() {
    for format in [NumberFormat::EnUs, NumberFormat::DeDe] {
        let fraction = if format == NumberFormat::EnUs {
            "12.5"
        } else {
            "12,5"
        };
        for (value, text) in [
            (Value::Int64(-12345), "-12345"),
            (Value::Currency(Currency::from_scaled(125_000)), fraction),
            (Value::Boolean(true), "True"),
            (
                Value::DateTime(LegacyDateTime::from_ole_days(2.0).unwrap()),
                "1900-01-01",
            ),
        ] {
            for function in [
                TextFunction::Asc,
                TextFunction::AscW,
                TextFunction::StrReverse,
            ] {
                let expected = if function == TextFunction::StrReverse {
                    Value::Text(text.chars().rev().collect())
                } else {
                    Value::Int16(i16::from(text.as_bytes()[0]))
                };
                let mut budget = ResourceBudget::new(1000, 0, 1000);
                assert_eq!(
                    Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                        .with_number_format(format)
                        .with_date_format(DateTextFormat::Iso)
                        .apply_text_function(function, std::slice::from_ref(&value)),
                    Ok(expected)
                );
            }
        }
    }
}

#[test]
fn character_and_reverse_null_empty_policy_and_surrogate_boundaries_remain_explicit() {
    let mut budget = ResourceBudget::new(1000, 0, 1000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    for function in [
        TextFunction::Asc,
        TextFunction::AscW,
        TextFunction::StrReverse,
    ] {
        assert_eq!(
            evaluator.apply_text_function(function, &[Value::Null]),
            Err(EvalError::InvalidNullConversion)
        );
        assert_eq!(
            evaluator.apply_text_function(function, &[Value::Int64(65)]),
            Err(EvalError::NumberFormatRequired)
        );
        assert_eq!(
            evaluator.apply_text_function(function, &[Value::Binary(vec![65])]),
            Err(EvalError::TypeMismatch)
        );
    }
    assert_eq!(
        evaluator.apply_text_function(TextFunction::Asc, &[Value::Text("".into())]),
        Err(EvalError::Domain)
    );
    assert_eq!(
        evaluator.apply_text_function(TextFunction::StrReverse, &[Value::Text("".into())]),
        Ok(Value::Text("".into()))
    );
    assert_eq!(
        evaluator.apply_text_function(TextFunction::Asc, &[Value::Text("é".into())]),
        Err(EvalError::CharacterPolicyRequired)
    );
    assert_eq!(
        evaluator.apply_text_function(TextFunction::AscW, &[Value::Text("🦀".into())]),
        Ok(Value::Int16(-10178))
    );
    assert_eq!(
        evaluator.apply_text_function(TextFunction::StrReverse, &[Value::Text("🦀".into())]),
        Err(EvalError::UnrepresentableText)
    );
}

#[test]
fn character_and_reverse_conversion_respect_bytes_and_every_cancellation_checkpoint() {
    for (function, allowance, succeeds) in [
        (TextFunction::Asc, 4, false),
        (TextFunction::Asc, 5, true),
        (TextFunction::StrReverse, 9, false),
        (TextFunction::StrReverse, 10, true),
    ] {
        let mut budget = ResourceBudget::new(allowance, 0, 1000);
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_number_format(NumberFormat::EnUs)
                .apply_text_function(function, &[Value::Int64(12345)])
                .is_ok(),
            succeeds
        );
    }
    for function in [
        TextFunction::Asc,
        TextFunction::AscW,
        TextFunction::StrReverse,
    ] {
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
                .apply_text_function(function, &[Value::Int64(12345)])
        };
        let expected = if function == TextFunction::StrReverse {
            Value::Text("54321".into())
        } else {
            Value::Int16(49)
        };
        assert_eq!(invoke(usize::MAX), Ok(expected));
        let observed = calls.get();
        for stop in 1..=observed {
            assert_eq!(invoke(stop), Err(EvalError::Cancelled));
        }
    }
}

#[test]
fn val_formats_scalar_inputs_then_keeps_period_only_prefix_parsing() {
    for (format, fraction) in [(NumberFormat::EnUs, 12.5), (NumberFormat::DeDe, 12.0)] {
        for (value, expected) in [
            (Value::Currency(Currency::from_scaled(125_000)), fraction),
            (Value::Boolean(true), 0.0),
            (
                Value::DateTime(LegacyDateTime::from_ole_days(2.0).unwrap()),
                1900.0,
            ),
            (Value::Text("12.5".into()), 12.5),
        ] {
            let mut budget = ResourceBudget::new(1000, 0, 1000);
            assert_eq!(
                Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                    .with_number_format(format)
                    .with_date_format(DateTextFormat::Iso)
                    .apply_text_function(TextFunction::Val, &[value]),
                Ok(Value::Double(Double::from_float(expected)))
            );
        }
    }
    let mut budget = ResourceBudget::new(1000, 0, 1000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    assert_eq!(
        evaluator.apply_text_function(TextFunction::Val, &[Value::Null]),
        Err(EvalError::InvalidNullConversion)
    );
    assert_eq!(
        evaluator.apply_text_function(TextFunction::Val, &[Value::Int64(123)]),
        Err(EvalError::NumberFormatRequired)
    );
    assert_eq!(
        evaluator.apply_text_function(TextFunction::Val, &[Value::Boolean(false)]),
        Ok(Value::Double(Double::from_float(0.0)))
    );
    assert_eq!(
        evaluator.apply_text_function(TextFunction::Val, &[Value::Binary(vec![1])]),
        Err(EvalError::TypeMismatch)
    );
}

#[test]
fn val_scalar_formatting_and_prefix_normalization_share_quotas_and_cancellation() {
    for (allowance, succeeds) in [(9, false), (10, true)] {
        let mut budget = ResourceBudget::new(allowance, 0, 1000);
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_number_format(NumberFormat::EnUs)
                .apply_text_function(TextFunction::Val, &[Value::Int64(12345)])
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
            .apply_text_function(TextFunction::Val, &[Value::Int64(12345)])
    };
    assert_eq!(
        invoke(usize::MAX),
        Ok(Value::Double(Double::from_float(12345.0)))
    );
    let observed = calls.get();
    for stop in 1..=observed {
        assert_eq!(invoke(stop), Err(EvalError::Cancelled));
    }
}

use super::*;
use crate::{
    evaluation::{EvalLimits, NoBindings},
    functions::{
        conversions::text::NumberFormat,
        text::{TextFunction, cstr::DateTextFormat},
    },
};
use std::cell::Cell;
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Currency, LegacyDateTime},
};

#[test]
fn scalar_inputs_use_shared_string_formats_in_all_three_positions() {
    for format in [NumberFormat::EnUs, NumberFormat::DeDe] {
        let separator = if format == NumberFormat::EnUs {
            "."
        } else {
            ","
        };
        let cases = [
            (
                vec![Value::Int64(123123), Value::Int16(23), Value::Int16(45)],
                "145145",
            ),
            (
                vec![
                    Value::Boolean(false),
                    Value::Text("al".into()),
                    Value::Int16(1),
                ],
                "F1se",
            ),
            (
                vec![
                    Value::Currency(Currency::from_scaled(125_000)),
                    Value::Text(separator.into()),
                    Value::Text("X".into()),
                ],
                "12X5",
            ),
            (
                vec![
                    Value::DateTime(LegacyDateTime::from_ole_days(2.0).unwrap()),
                    Value::Int16(1900),
                    Value::Int16(2000),
                ],
                "2000-01-01",
            ),
        ];
        for (arguments, expected) in cases {
            let mut budget = ResourceBudget::new(1000, 0, 10_000);
            let actual = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_number_format(format)
                .with_date_format(DateTextFormat::Iso)
                .apply_text_function(TextFunction::Replace, &arguments);
            assert_eq!(actual, Ok(Value::Text(expected.into())));
        }
    }
}

#[test]
fn options_and_null_errors_precede_unused_operand_shortcuts() {
    let mut budget = ResourceBudget::new(1000, 0, 1000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    for (arguments, error) in [
        (
            vec![Value::Null, Value::Int16(1), Value::Int16(2)],
            EvalError::InvalidNullConversion,
        ),
        (
            vec![
                Value::Null,
                Value::Int16(1),
                Value::Int16(2),
                Value::Int16(0),
            ],
            EvalError::Domain,
        ),
        (
            vec![
                Value::Null,
                Value::Int16(1),
                Value::Int16(2),
                Value::Int16(1),
                Value::Int16(-1),
                Value::Int16(3),
            ],
            EvalError::Domain,
        ),
        (
            vec![
                Value::Text("a".into()),
                Value::Text("a".into()),
                Value::Int16(2),
                Value::Int16(1),
                Value::Int16(0),
            ],
            EvalError::NumberFormatRequired,
        ),
        (
            vec![Value::Text("".into()), Value::Text("a".into()), Value::Null],
            EvalError::InvalidNullConversion,
        ),
        (
            vec![
                Value::Binary(vec![1]),
                Value::Text("a".into()),
                Value::Text("b".into()),
            ],
            EvalError::TypeMismatch,
        ),
    ] {
        assert_eq!(
            evaluator.apply_text_function(TextFunction::Replace, &arguments),
            Err(error)
        );
    }
}

#[test]
fn scalar_conversion_and_output_share_text_bytes_work_and_cancellation() {
    let arguments = [Value::Int64(123123), Value::Int16(23), Value::Int16(45)];
    let mut budget = ResourceBudget::new(10, 0, 1000);
    assert!(matches!(
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_number_format(NumberFormat::EnUs)
            .apply_text_function(TextFunction::Replace, &arguments),
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
        .apply_text_function(TextFunction::Replace, &arguments),
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
            .apply_text_function(TextFunction::Replace, &arguments)
    };
    assert_eq!(invoke(usize::MAX), Ok(Value::Text("145145".into())));
    let observed = calls.get();
    for stop in 1..=observed {
        assert_eq!(invoke(stop), Err(EvalError::Cancelled));
    }
}

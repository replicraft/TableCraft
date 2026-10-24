use super::*;
use crate::{
    evaluation::{EvalLimits, NoBindings},
    functions::conversions::text::NumberFormat,
};
use std::cell::Cell;
use tablecraft_core::{limits::ResourceBudget, values::ExpressionString};

fn retained(text: &str) -> ExpressionValue {
    ExpressionValue::StringBytes(ExpressionString::from_owned(
        text.encode_utf16().flat_map(u16::to_le_bytes).collect(),
    ))
}

fn render(
    evaluator: &mut Evaluator<'_>,
    function: TextFunction,
    value: &ExpressionValue,
) -> Result<Value, EvalError> {
    if function == TextFunction::Str {
        evaluator.apply_expression_str(value)
    } else {
        evaluator.apply_expression_radix(function, value)
    }
}

#[test]
fn retained_numeric_strings_share_checked_conversion_and_rendering() {
    for (format, input, expected) in [
        (NumberFormat::EnUs, "2.5", ["2", "2", " 2.5"]),
        (NumberFormat::EnUs, "3.5", ["4", "4", " 3.5"]),
        (NumberFormat::DeDe, "1.234,5", ["4D2", "2322", " 1234.5"]),
    ] {
        for (function, expected) in [TextFunction::Hex, TextFunction::Oct, TextFunction::Str]
            .into_iter()
            .zip(expected)
        {
            let mut budget = ResourceBudget::new(65536, 0, 10000);
            let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_number_format(format);
            let value = retained(input);
            assert_eq!(
                render(&mut evaluator, function, &value),
                Ok(Value::Text(expected.into()))
            );
            assert_eq!(value, retained(input));
        }
    }
}

#[test]
fn retained_ranges_validate_only_active_units_and_keep_failures_explicit() {
    let raw = ExpressionString::from_owned(vec![0, 216, b'1', 0, b'2', 0, 255]);
    let selected = ExpressionValue::StringBytes(raw.select(2..6).unwrap());
    for function in [TextFunction::Hex, TextFunction::Oct, TextFunction::Str] {
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
        assert_eq!(
            render(&mut evaluator, function, &selected),
            Err(EvalError::NumberFormatRequired)
        );
        let mut evaluator = evaluator.with_number_format(NumberFormat::EnUs);
        let expected = match function {
            TextFunction::Hex => "C",
            TextFunction::Oct => "14",
            TextFunction::Str => " 12",
            _ => unreachable!(),
        };
        assert_eq!(
            render(&mut evaluator, function, &selected),
            Ok(Value::Text(expected.into()))
        );
        for bytes in [vec![0, 216], vec![b'1', 0, 255]] {
            let value = ExpressionValue::StringBytes(ExpressionString::from_owned(bytes));
            assert_eq!(
                render(&mut evaluator, function, &value),
                Err(EvalError::UnrepresentableText)
            );
        }
        assert_eq!(
            render(&mut evaluator, function, &retained("invalid")),
            Err(EvalError::TypeMismatch)
        );
        if function != TextFunction::Str {
            assert_eq!(
                render(&mut evaluator, function, &retained("2147483648")),
                Err(EvalError::Overflow)
            );
        }
    }
}

#[test]
fn retained_numeric_conversion_accounts_for_publication_work_and_allocation() {
    for function in [TextFunction::Hex, TextFunction::Oct, TextFunction::Str] {
        let value = retained("1234.5");
        let calls = Cell::new(0);
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        render(
            &mut Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_number_format(NumberFormat::EnUs)
                .with_cancellation(&|| {
                    calls.set(calls.get() + 1);
                    false
                }),
            function,
            &value,
        )
        .unwrap();
        for stop in 1..=calls.get() {
            let observed = Cell::new(0);
            let mut budget = ResourceBudget::new(65536, 0, 10000);
            assert_eq!(
                render(
                    &mut Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                        .with_number_format(NumberFormat::EnUs)
                        .with_cancellation(&|| {
                            observed.set(observed.get() + 1);
                            observed.get() >= stop
                        }),
                    function,
                    &value,
                ),
                Err(EvalError::Cancelled)
            );
        }
        for (bytes, steps) in [(0, 10000), (65536, 0)] {
            let mut budget = ResourceBudget::new(bytes, 0, steps);
            assert!(matches!(
                render(
                    &mut Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                        .with_number_format(NumberFormat::EnUs),
                    function,
                    &value,
                ),
                Err(EvalError::Limit(_))
            ));
        }
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        assert_eq!(
            render(
                &mut Evaluator::new(
                    &NoBindings,
                    &mut budget,
                    EvalLimits {
                        max_text_bytes: 2,
                        ..Default::default()
                    }
                )
                .with_number_format(NumberFormat::EnUs),
                function,
                &value,
            ),
            Err(EvalError::TextLimit)
        );
    }
}

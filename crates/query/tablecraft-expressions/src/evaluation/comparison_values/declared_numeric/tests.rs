use super::*;
use crate::{evaluation::NoBindings, functions::conversions::text::NumberFormat};
use std::cell::Cell;
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Double, ExpressionString, Single},
};

#[test]
fn selected_numeric_width_preserves_rounding_precision_and_operand_direction() {
    for (numeric, text, equal) in [
        (Value::Boolean(true), "2", true),
        (Value::Byte(2), "2.5", true),
        (Value::Int16(4), "3.5", true),
        (Value::Int32(-2), "-2.5", true),
        (
            Value::Int64(9_007_199_254_740_993),
            "9007199254740992",
            false,
        ),
        (
            Value::Int64(9_007_199_254_740_993),
            "9007199254740993",
            true,
        ),
        (
            Value::Single(Single::from_float(16_777_216.0)),
            "16777217",
            true,
        ),
        (Value::Double(Double::from_float(0.5)), "0.5", true),
    ] {
        let numeric = ExpressionValue::Value(numeric);
        for raw in [false, true] {
            let string = if raw {
                ExpressionValue::StringBytes(ExpressionString::from_owned(
                    text.encode_utf16().flat_map(u16::to_le_bytes).collect(),
                ))
            } else {
                ExpressionValue::Value(Value::Text(text.into()))
            };
            for string_declaration in [
                None,
                Some(OperandDeclaration::Scalar),
                Some(OperandDeclaration::Variant),
            ] {
                for reverse in [false, true] {
                    let (left, right, a, b) = if reverse {
                        (
                            &string,
                            &numeric,
                            string_declaration,
                            Some(OperandDeclaration::Scalar),
                        )
                    } else {
                        (
                            &numeric,
                            &string,
                            Some(OperandDeclaration::Scalar),
                            string_declaration,
                        )
                    };
                    let mut budget = ResourceBudget::new(65_536, 0, 10_000);
                    let result = Evaluator::new(&NoBindings, &mut budget, Default::default())
                        .with_number_format(NumberFormat::Invariant)
                        .compare_scalar_numeric_string(Comparison::Equal, left, right, a, b);
                    assert_eq!(
                        result,
                        Ok(Some(if equal { Truth::True } else { Truth::False })),
                        "{numeric:?}, {text}, raw={raw}, reverse={reverse}"
                    );
                }
            }
        }
    }
}

#[test]
fn selected_conversion_keeps_range_policy_and_decline_boundaries() {
    for (numeric, text, expected) in [
        (Value::Byte(255), "255.5", EvalError::Overflow),
        (Value::Int16(0), "32768", EvalError::Overflow),
        (Value::Int32(0), "2147483648", EvalError::Overflow),
        (Value::Int64(0), "9223372036854775808", EvalError::Overflow),
        (Value::Int16(2), "bad", EvalError::TypeMismatch),
    ] {
        let mut budget = ResourceBudget::new(65_536, 0, 10_000);
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, Default::default())
                .with_number_format(NumberFormat::Invariant)
                .compare_scalar_numeric_string(
                    Comparison::Equal,
                    &ExpressionValue::Value(numeric),
                    &ExpressionValue::Value(Value::Text(text.into())),
                    Some(OperandDeclaration::Scalar),
                    None
                ),
            Err(expected)
        );
    }

    let numeric = ExpressionValue::Value(Value::Int16(2));
    let string = ExpressionValue::Value(Value::Text("2,5".into()));
    let mut budget = ResourceBudget::new(65_536, 0, 10_000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, Default::default());
    assert_eq!(
        evaluator.compare_scalar_numeric_string(
            Comparison::Equal,
            &numeric,
            &string,
            Some(OperandDeclaration::Scalar),
            None
        ),
        Err(EvalError::NumberFormatRequired)
    );
    assert_eq!(
        evaluator.compare_scalar_numeric_string(
            Comparison::Equal,
            &numeric,
            &string,
            Some(OperandDeclaration::Variant),
            None
        ),
        Ok(None)
    );
    assert_eq!(
        evaluator.compare_scalar_numeric_string(
            Comparison::Equal,
            &numeric,
            &string,
            None,
            Some(OperandDeclaration::Scalar)
        ),
        Ok(None)
    );
    assert_eq!(
        evaluator
            .with_number_format(NumberFormat::DeDe)
            .compare_scalar_numeric_string(
                Comparison::Equal,
                &numeric,
                &string,
                Some(OperandDeclaration::Scalar),
                None
            ),
        Ok(Some(Truth::True))
    );
}

#[test]
fn normalization_and_every_observed_checkpoint_use_the_existing_budget() {
    let numeric = ExpressionValue::Value(Value::Int64(9_007_199_254_740_993));
    let text = ExpressionValue::Value(Value::Text("9007199254740993".into()));
    let observed = Cell::new(0);
    let count = || {
        observed.set(observed.get() + 1);
        false
    };
    let mut budget = ResourceBudget::new(65_536, 0, 10_000);
    let result = Evaluator::new(&NoBindings, &mut budget, Default::default())
        .with_number_format(NumberFormat::Invariant)
        .with_cancellation(&count)
        .compare_scalar_numeric_string(
            Comparison::Equal,
            &numeric,
            &text,
            Some(OperandDeclaration::Scalar),
            None,
        );
    assert_eq!(result, Ok(Some(Truth::True)));
    assert!(observed.get() > 0);

    for stop in 1..=observed.get() {
        let calls = Cell::new(0);
        let cancelled = || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        };
        let mut budget = ResourceBudget::new(65_536, 0, 10_000);
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, Default::default())
                .with_number_format(NumberFormat::Invariant)
                .with_cancellation(&cancelled)
                .compare_scalar_numeric_string(
                    Comparison::Equal,
                    &numeric,
                    &text,
                    Some(OperandDeclaration::Scalar),
                    None
                ),
            Err(EvalError::Cancelled)
        );
    }

    let mut budget = ResourceBudget::new(0, 0, 10_000);
    assert!(matches!(
        Evaluator::new(&NoBindings, &mut budget, Default::default())
            .with_number_format(NumberFormat::Invariant)
            .compare_scalar_numeric_string(
                Comparison::Equal,
                &numeric,
                &text,
                Some(OperandDeclaration::Scalar),
                None
            ),
        Err(EvalError::Limit(_))
    ));
}

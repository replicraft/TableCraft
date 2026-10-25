use super::*;
use crate::{
    comparison::{Collation, StringComparisonProfile},
    evaluation::NoBindings,
    functions::conversions::text::NumberFormat,
};
use std::{cell::Cell, cmp::Ordering};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Currency, Decimal, Double, ExpressionString, LegacyDateTime, Single},
};

struct Ordinal;
impl Collation for Ordinal {
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        Ok(left.cmp(right))
    }
}

fn declarations(reverse: bool) -> ArithmeticDeclarations {
    if reverse {
        ArithmeticDeclarations {
            left: OperandDeclaration::Variant,
            right: OperandDeclaration::Scalar,
        }
    } else {
        ArithmeticDeclarations {
            left: OperandDeclaration::Scalar,
            right: OperandDeclaration::Variant,
        }
    }
}

#[test]
fn scalar_string_formats_every_supported_numeric_variant_in_both_orders() {
    let string = ExpressionValue::Value(Value::Text("02".into()));
    for numeric in [
        Value::Byte(2),
        Value::Int16(2),
        Value::Int32(2),
        Value::Int64(2),
        Value::Single(Single::from_float(2.0)),
        Value::Double(Double::from_float(2.0)),
        Value::Currency(Currency::from_scaled(20_000)),
        Value::Decimal(Decimal::new(2, 0).unwrap()),
    ] {
        let peer = ExpressionValue::Value(numeric);
        for reverse in [false, true] {
            let (left, right) = if reverse {
                (&peer, &string)
            } else {
                (&string, &peer)
            };
            let mut budget = ResourceBudget::new(65_536, 0, 10_000);
            let mut evaluator = Evaluator::new(&NoBindings, &mut budget, Default::default())
                .with_number_format(NumberFormat::Invariant)
                .with_collation(&Ordinal);
            assert_eq!(
                evaluator.compare_scalar_string_variant(
                    Comparison::Equal,
                    left,
                    right,
                    declarations(reverse)
                ),
                Ok(Some(Truth::False))
            );
            assert_eq!(
                evaluator.compare_scalar_string_variant(
                    Comparison::Less,
                    left,
                    right,
                    declarations(reverse)
                ),
                Ok(Some(if reverse { Truth::False } else { Truth::True }))
            );
        }
    }
}

#[test]
fn booleans_dates_empty_and_expression_errors_keep_their_own_boundaries() {
    for (text, peer, expected) in [
        (
            "True",
            ExpressionValue::Value(Value::Boolean(true)),
            Ok(Some(Truth::True)),
        ),
        ("", ExpressionValue::Empty, Ok(Some(Truth::True))),
        (
            "x",
            ExpressionValue::Value(Value::Null),
            Ok(Some(Truth::Unknown)),
        ),
        ("x", ExpressionValue::Error(1), Err(EvalError::TypeMismatch)),
        (
            "x",
            ExpressionValue::Missing,
            Err(EvalError::NonScalarExpression),
        ),
        (
            "x",
            ExpressionValue::Value(Value::Binary(vec![])),
            Err(EvalError::TypeMismatch),
        ),
        (
            "x",
            ExpressionValue::Value(Value::DateTime(LegacyDateTime::from_ole_days(0.0).unwrap())),
            Err(EvalError::DateFormatRequired),
        ),
    ] {
        let mut budget = ResourceBudget::new(65_536, 0, 10_000);
        let result = Evaluator::new(&NoBindings, &mut budget, Default::default())
            .with_number_format(NumberFormat::Invariant)
            .with_collation(&Ordinal)
            .compare_scalar_string_variant(
                Comparison::Equal,
                &ExpressionValue::Value(Value::Text(text.into())),
                &peer,
                declarations(false),
            );
        assert_eq!(result, expected);
    }
}

#[test]
fn raw_string_and_policy_failures_are_not_numeric_parsing_or_error_formatting() {
    let raw = ExpressionValue::StringBytes(ExpressionString::from_owned(vec![0, 216]));
    let two = ExpressionValue::Value(Value::Int32(2));
    let mut budget = ResourceBudget::new(65_536, 0, 10_000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, Default::default());
    assert_eq!(
        evaluator.compare_scalar_string_variant(Comparison::Equal, &raw, &two, declarations(false)),
        Err(EvalError::NumberFormatRequired)
    );
    let mut evaluator = evaluator
        .with_number_format(NumberFormat::Invariant)
        .with_collation(&Ordinal);
    assert_eq!(
        evaluator.compare_scalar_string_variant(Comparison::Equal, &raw, &two, declarations(false)),
        Err(EvalError::UnrepresentableText)
    );
    assert_eq!(
        evaluator.compare_scalar_string_variant(
            Comparison::Equal,
            &raw,
            &ExpressionValue::Error(1),
            declarations(false)
        ),
        Err(EvalError::TypeMismatch)
    );
    assert_eq!(
        evaluator.compare_scalar_string_variant(
            Comparison::Equal,
            &raw,
            &ExpressionValue::Value(Value::Null),
            declarations(false)
        ),
        Ok(Some(Truth::Unknown))
    );
    assert_eq!(
        evaluator.compare_scalar_string_variant(
            Comparison::Equal,
            &raw,
            &two,
            ArithmeticDeclarations::default()
        ),
        Ok(None)
    );
}

#[test]
fn formatting_allocations_and_every_observed_cancellation_remain_shared() {
    let text = ExpressionValue::Value(Value::Text("02".into()));
    let numeric = ExpressionValue::Value(Value::Int32(2));
    let observed = Cell::new(0);
    let count = || {
        observed.set(observed.get() + 1);
        false
    };
    let mut budget = ResourceBudget::new(65_536, 0, 10_000);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, Default::default())
            .with_number_format(NumberFormat::Invariant)
            .with_collation(&Ordinal)
            .with_cancellation(&count)
            .compare_scalar_string_variant(Comparison::Equal, &text, &numeric, declarations(false)),
        Ok(Some(Truth::False))
    );
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
                .with_collation(&Ordinal)
                .with_cancellation(&cancelled)
                .compare_scalar_string_variant(
                    Comparison::Equal,
                    &text,
                    &numeric,
                    declarations(false)
                ),
            Err(EvalError::Cancelled)
        );
    }
    let mut budget = ResourceBudget::new(0, 0, 10_000);
    assert!(matches!(
        Evaluator::new(&NoBindings, &mut budget, Default::default())
            .with_number_format(NumberFormat::Invariant)
            .with_collation(&Ordinal)
            .compare_scalar_string_variant(Comparison::Equal, &text, &numeric, declarations(false)),
        Err(EvalError::Limit(_))
    ));
}

#[test]
fn explicit_raw_provider_compares_only_active_units_without_unicode_publication() {
    struct Raw;
    impl Collation for Raw {
        fn compare(&self, _: &str, _: &str) -> Result<Ordering, EvalError> {
            Err(EvalError::Domain)
        }

        fn string_comparison_profile(&self) -> StringComparisonProfile {
            StringComparisonProfile::Utf16
        }

        fn compare_utf16(&self, left: &[u16], right: &[u16]) -> Result<Ordering, EvalError> {
            Ok(left.cmp(right))
        }
    }

    let raw = ExpressionValue::StringBytes(
        ExpressionString::from_owned(vec![255, 0, 216, 255])
            .select(1..3)
            .unwrap(),
    );
    let numeric = ExpressionValue::Value(Value::Int32(2));
    for reverse in [false, true] {
        let (left, right) = if reverse {
            (&numeric, &raw)
        } else {
            (&raw, &numeric)
        };
        let mut budget = ResourceBudget::new(65_536, 0, 10_000);
        let result = Evaluator::new(&NoBindings, &mut budget, Default::default())
            .with_number_format(NumberFormat::Invariant)
            .with_collation(&Raw)
            .compare_scalar_string_variant(Comparison::Greater, left, right, declarations(reverse));
        assert_eq!(
            result,
            Ok(Some(if reverse { Truth::False } else { Truth::True }))
        );
    }
}

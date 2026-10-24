use super::*;
use crate::{
    evaluation::{Evaluator, NoBindings, PreparedAddition},
    functions::{
        conversions::text::NumberFormat,
        text::byte_string::{ByteOperand, ByteString, ExpressionByteOperand},
    },
};
use proptest::prelude::*;
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Currency, Decimal, LegacyDateTime, Single},
};

fn apply(operator: B, left: ExpressionValue, right: ExpressionValue) -> Result<Value, EvalError> {
    Evaluator::new(
        &NoBindings,
        &mut ResourceBudget::new(65536, 0, 10000),
        Default::default(),
    )
    .with_number_format(NumberFormat::EnUs)
    .apply_expression_binary(operator, left, right)
}

#[test]
fn empty_arithmetic_selects_the_original_effective_type_without_manufacturing_dates() {
    for (operator, right, expected) in [
        (B::Add, Value::Byte(7), Value::Byte(7)),
        (B::Subtract, Value::Byte(7), Value::Int16(-7)),
        (B::Multiply, Value::Byte(7), Value::Byte(0)),
        (B::IntegerDivide, Value::Byte(7), Value::Int16(0)),
        (B::Modulo, Value::Byte(7), Value::Int16(0)),
        (
            B::IntegerDivide,
            Value::Single(Single::from_float(2.5)),
            Value::Int32(0),
        ),
        (
            B::Modulo,
            Value::Currency(Currency::from_scaled(20000)),
            Value::Int32(0),
        ),
        (
            B::Add,
            Value::Int64(9007199254740993),
            Value::Int64(9007199254740993),
        ),
        (
            B::Add,
            Value::Single(Single::from_float(1.5)),
            Value::Single(Single::from_float(1.5)),
        ),
        (
            B::Add,
            Value::Currency(Currency::from_scaled(12345)),
            Value::Currency(Currency::from_scaled(12345)),
        ),
        (
            B::Add,
            Value::Decimal(Decimal::new(12300, 2).unwrap()),
            Value::Decimal(Decimal::new(12300, 2).unwrap()),
        ),
        (
            B::Subtract,
            Value::DateTime(LegacyDateTime::from_ole_days(1.25).unwrap()),
            Value::DateTime(LegacyDateTime::from_ole_days(-1.25).unwrap()),
        ),
        (
            B::Add,
            Value::Text("12.5".into()),
            Value::Text("12.5".into()),
        ),
        (B::Add, Value::Null, Value::Null),
    ] {
        assert_eq!(
            apply(
                operator,
                ExpressionValue::Empty,
                ExpressionValue::Value(right)
            ),
            Ok(expected)
        );
    }
    for operator in [B::Add, B::Subtract, B::Multiply] {
        assert_eq!(
            apply(operator, ExpressionValue::Empty, ExpressionValue::Empty),
            Ok(Value::Int16(0))
        );
    }
    for operator in [U::Plus, U::Minus] {
        let value = arithmetic_unary_operand(operator, ExpressionValue::Empty).unwrap();
        let result = Evaluator::new(
            &NoBindings,
            &mut ResourceBudget::new(0, 0, 100),
            Default::default(),
        )
        .apply_unary(operator, value);
        assert_eq!(result, Ok(Value::Int16(0)));
    }
}

#[test]
fn empty_keeps_error_missing_family_and_numeric_policy_failures_visible() {
    for numerator in [
        Value::Single(Single::from_bits(0)),
        Value::Double(Double::from_bits(0x8000000000000000)),
        Value::DateTime(LegacyDateTime::from_ole_days(0.0).unwrap()),
        Value::Text("0".into()),
    ] {
        assert_eq!(
            apply(
                B::Divide,
                ExpressionValue::Value(numerator),
                ExpressionValue::Empty
            ),
            Err(EvalError::DivisionByZero)
        );
    }
    assert_eq!(
        apply(
            B::Divide,
            ExpressionValue::Value(Value::Int16(0)),
            ExpressionValue::Empty
        ),
        Err(EvalError::Overflow)
    );
    for (right, expected) in [
        (ExpressionValue::Error(0), EvalError::TypeMismatch),
        (ExpressionValue::Missing, EvalError::NonScalarExpression),
        (
            ExpressionValue::Value(Value::Binary(vec![])),
            EvalError::TypeMismatch,
        ),
        (
            ExpressionValue::Value(Value::Double(Double::from_float(f64::INFINITY))),
            EvalError::NonFinite,
        ),
    ] {
        assert_eq!(apply(B::Add, ExpressionValue::Empty, right), Err(expected));
    }
    for operator in [B::Divide, B::IntegerDivide, B::Modulo] {
        assert_eq!(
            apply(
                operator,
                ExpressionValue::Value(Value::Byte(1)),
                ExpressionValue::Empty
            ),
            Err(EvalError::DivisionByZero)
        );
    }
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, Default::default());
    assert_eq!(
        evaluator.apply_expression_binary(
            B::Subtract,
            ExpressionValue::Empty,
            ExpressionValue::Value(Value::Text("12".into()))
        ),
        Err(EvalError::NumberFormatRequired)
    );
    assert_eq!(
        arithmetic_unary_operand(U::Not, ExpressionValue::Empty),
        Err(EvalError::UnsupportedExpression)
    );
    assert!(arithmetic_operands(B::Equal, ExpressionValue::Empty, ExpressionValue::Empty).is_err());
}

#[test]
fn empty_plus_a_raw_string_retains_string_identity_without_numeric_conversion() {
    for bytes in [vec![49, 0, 50, 0], vec![0, 216], vec![65]] {
        for empty_on_left in [true, false] {
            let raw = ByteString::from_owned(bytes.clone()).unwrap();
            let pointer = raw.as_bytes().as_ptr();
            let raw = ExpressionByteOperand::Bytes(raw);
            let empty = ExpressionByteOperand::Scalar(ExpressionValue::Empty);
            let (left, right) = if empty_on_left {
                (empty, raw)
            } else {
                (raw, empty)
            };
            let mut budget = ResourceBudget::new(0, 0, 100);
            let PreparedAddition::StringResult(ByteOperand::Bytes(result)) =
                Evaluator::new(&NoBindings, &mut budget, Default::default())
                    .prepare_expression_addition(left, right)
                    .unwrap()
            else {
                panic!("raw string identity required");
            };
            assert_eq!(result.as_bytes(), bytes);
            assert_eq!(result.as_bytes().as_ptr(), pointer);
        }
    }
}

proptest! {
    #![proptest_config(crate::test_support::config())]

    #[test]
    fn empty_preserves_byte_addition_and_rounded_quotient_exceptions(byte in 1u8..=255) {
        let value = ExpressionValue::Value(Value::Byte(byte));
        prop_assert_eq!(apply(B::Add, ExpressionValue::Empty, value.clone()), Ok(Value::Byte(byte)));
        prop_assert_eq!(apply(B::Multiply, value.clone(), ExpressionValue::Empty), Ok(Value::Byte(0)));
        prop_assert_eq!(apply(B::IntegerDivide, ExpressionValue::Empty, value.clone()), Ok(Value::Int16(0)));
        prop_assert_eq!(apply(B::Modulo, ExpressionValue::Empty, value), Ok(Value::Int16(0)));
    }
}

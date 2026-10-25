use super::*;
use crate::{
    evaluation::{EvalLimits, NoBindings, PreparedNumericArithmetic},
    functions::{
        conversions::text::NumberFormat,
        text::byte_string::{ByteString, ExpressionByteOperand},
    },
};
use std::cell::Cell;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{Decimal, Double, ExpressionValue},
};
use tablecraft_sql::select::BinaryOperator;

fn decimal() -> Value {
    Value::Decimal(Decimal::new(1, 0).unwrap())
}

#[test]
fn declared_decimal_text_division_uses_double_before_text_conversion() {
    for text in ["1e-100", "1e100", "0,5"] {
        for reverse in [false, true] {
            let (left, right) = if reverse {
                (Value::Text(text.into()), decimal())
            } else {
                (decimal(), Value::Text(text.into()))
            };
            let value: f64 = match text {
                "0,5" => 0.5,
                other => other.parse().unwrap(),
            };
            let expected = Value::Double(Double::from_float(if reverse {
                value
            } else {
                1.0 / value
            }));
            let mut budget = ResourceBudget::new(65_536, 0, 65_536);
            let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_number_format(NumberFormat::DeDe);
            assert_eq!(
                evaluator.apply_arithmetic_agreement(Arithmetic::Divide, &left, &right),
                Ok(Some(expected.clone()))
            );
            assert_eq!(
                evaluator.apply_declared_numeric_arithmetic(
                    Arithmetic::Divide,
                    &left,
                    &right,
                    ArithmeticDeclarations::default()
                ),
                Ok(expected)
            );
        }
    }
}

#[test]
fn text_policy_null_and_admission_precedence_stay_explicit() {
    let mut budget = ResourceBudget::new(65_536, 0, 65_536);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    assert_eq!(
        evaluator.apply_arithmetic_agreement(
            Arithmetic::Divide,
            &decimal(),
            &Value::Text("3".into())
        ),
        Err(EvalError::NumberFormatRequired)
    );
    assert_eq!(
        evaluator.apply_arithmetic_agreement(
            Arithmetic::Divide,
            &Value::Null,
            &Value::Text("invalid".into())
        ),
        Ok(Some(Value::Null))
    );
    assert_eq!(
        evaluator.apply_arithmetic_agreement(
            Arithmetic::Divide,
            &Value::Null,
            &Value::Binary(vec![0])
        ),
        Err(EvalError::TypeMismatch)
    );
}

#[test]
fn declared_text_division_is_cancellable_and_charges_each_execution() {
    let checks = Cell::new(0);
    let cancelled = || {
        checks.set(checks.get() + 1);
        false
    };
    let mut budget = ResourceBudget::new(65_536, 0, 65_536);
    let left = decimal();
    let right = Value::Text("0,5".into());
    Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .with_number_format(NumberFormat::DeDe)
        .with_cancellation(&cancelled)
        .apply_arithmetic_agreement(Arithmetic::Divide, &left, &right)
        .unwrap();
    assert!(budget.remaining(Resource::Bytes) < 65_536);
    for stop in 1..=checks.get() {
        let current = Cell::new(0);
        let mut budget = ResourceBudget::new(65_536, 0, 65_536);
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_number_format(NumberFormat::DeDe)
                .with_cancellation(&|| {
                    current.set(current.get() + 1);
                    current.get() >= stop
                })
                .apply_arithmetic_agreement(Arithmetic::Divide, &left, &right),
            Err(EvalError::Cancelled)
        );
    }
    let mut budget = ResourceBudget::new(0, 0, 65_536);
    assert!(matches!(
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_number_format(NumberFormat::DeDe)
            .apply_arithmetic_agreement(Arithmetic::Divide, &left, &right),
        Err(EvalError::Limit(_))
    ));
}

#[test]
fn raw_decimal_division_retains_publication_before_contextual_conversion() {
    for (bytes, expected) in [
        (
            vec![51, 0],
            Ok(Value::Double(Double::from_float(1.0 / 3.0))),
        ),
        (vec![0, 216], Err(EvalError::UnrepresentableText)),
        (vec![51], Err(EvalError::UnrepresentableText)),
    ] {
        let mut budget = ResourceBudget::new(65_536, 0, 65_536);
        let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_number_format(NumberFormat::EnUs);
        let result = evaluator
            .prepare_expression_numeric_arithmetic(
                BinaryOperator::Divide,
                ExpressionByteOperand::Scalar(ExpressionValue::Value(decimal())),
                ExpressionByteOperand::Bytes(ByteString::from_owned(bytes).unwrap()),
            )
            .and_then(|prepared| match prepared {
                PreparedNumericArithmetic::Operands { left, right } => evaluator
                    .apply_declared_numeric_arithmetic(
                        Arithmetic::Divide,
                        &left,
                        &right,
                        ArithmeticDeclarations::default(),
                    ),
                PreparedNumericArithmetic::Result(value) => Ok(value),
            });

        assert_eq!(result, expected);
    }
}

#[test]
fn default_contextual_decimal_division_remains_decimal_first() {
    let mut budget = ResourceBudget::new(65_536, 0, 65_536);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .with_number_format(NumberFormat::EnUs);

    assert!(matches!(
        evaluator.numeric_binary(Arithmetic::Divide, &decimal(), &Value::Text("3".into())),
        Ok(Value::Decimal(_))
    ));
    assert_eq!(
        evaluator.numeric_binary(
            Arithmetic::Divide,
            &decimal(),
            &Value::Text("1e-100".into())
        ),
        Err(EvalError::DivisionByZero)
    );
}

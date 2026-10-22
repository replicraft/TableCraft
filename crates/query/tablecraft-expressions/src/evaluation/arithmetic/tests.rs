use super::*;
use crate::{
    evaluation::{EvalLimits, NoBindings},
    functions::conversions::text::NumberFormat,
};
use proptest::prelude::*;
use std::cell::Cell;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{Currency, Decimal, Double, LegacyDateTime},
};
use tablecraft_sql::select::{BinaryOperator as B, UnaryOperator as U};

fn double(value: f64) -> Value {
    Value::Double(Double::from_float(value))
}

#[test]
fn numeric_text_uses_operator_specific_exact_decimal_currency_and_date_paths() {
    let mut budget = ResourceBudget::new(65536, 0, 65536);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .with_number_format(NumberFormat::DeDe);
    for (operator, left, right, expected) in [
        (
            B::Add,
            Value::Text("2,5".into()),
            Value::Int16(3),
            double(5.5),
        ),
        (
            B::Subtract,
            Value::Int16(3),
            Value::Text("2,5".into()),
            double(0.5),
        ),
        (
            B::Multiply,
            Value::Text("2,5".into()),
            Value::Text("4".into()),
            double(10.0),
        ),
        (
            B::Divide,
            Value::Text("2,5".into()),
            Value::Int32(2),
            double(1.25),
        ),
        (
            B::Power,
            Value::Text("2".into()),
            Value::Int32(3),
            double(8.0),
        ),
        (
            B::IntegerDivide,
            Value::Text("9007199254740993".into()),
            Value::Int64(1),
            Value::Int64(9007199254740993),
        ),
        (
            B::Modulo,
            Value::Text("9,5".into()),
            Value::Text("2,5".into()),
            Value::Int32(0),
        ),
        (
            B::Add,
            Value::Text("0,00005".into()),
            Value::Currency(Currency::from_scaled(1)),
            Value::Currency(Currency::from_scaled(1)),
        ),
        (
            B::Multiply,
            Value::Currency(Currency::from_scaled(12500)),
            Value::Text("2".into()),
            double(2.5),
        ),
        (
            B::Add,
            Value::Text("9007199254740993,125".into()),
            Value::Decimal(Decimal::new(1, 0).unwrap()),
            Value::Decimal(Decimal::new(9007199254740994125, 3).unwrap()),
        ),
        (
            B::Add,
            Value::DateTime(LegacyDateTime::from_ole_days(-1.25).unwrap()),
            Value::Text("0,5".into()),
            Value::DateTime(LegacyDateTime::from_ole_days(-0.75).unwrap()),
        ),
        (
            B::Multiply,
            Value::DateTime(LegacyDateTime::from_ole_days(-1.25).unwrap()),
            Value::Text("2".into()),
            double(-2.5),
        ),
    ] {
        assert_eq!(
            evaluator.apply_binary(operator, &left, &right),
            Ok(expected)
        );
    }
    assert_eq!(
        evaluator.apply_unary(U::Minus, Value::Text("2,5".into())),
        Ok(double(-2.5))
    );
    assert_eq!(
        evaluator.apply_unary(U::Plus, Value::Text("2,5".into())),
        Ok(double(2.5))
    );
}

#[test]
fn concatenation_null_policy_and_numeric_failures_remain_distinct() {
    let mut budget = ResourceBudget::new(65536, 0, 65536);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    assert_eq!(
        evaluator.apply_binary(B::Add, &Value::Text("bad".into()), &Value::Text("2".into())),
        Ok(Value::Text("bad2".into()))
    );
    assert_eq!(
        evaluator.apply_binary(B::Add, &Value::Text("bad".into()), &Value::Null),
        Ok(Value::Null)
    );
    assert_eq!(
        evaluator.apply_binary(B::Multiply, &Value::Null, &Value::Text("bad".into())),
        Ok(Value::Null)
    );
    assert_eq!(
        evaluator.apply_binary(B::Add, &Value::Text("2".into()), &Value::Int16(1)),
        Err(EvalError::NumberFormatRequired)
    );
    let mut evaluator = evaluator.with_number_format(NumberFormat::EnUs);
    assert_eq!(
        evaluator.apply_binary(
            B::IntegerDivide,
            &Value::Text("2.5000000000000000000000000000000000001".into()),
            &Value::Int16(1)
        ),
        Ok(Value::Int32(3))
    );
    assert_eq!(
        evaluator.apply_binary(
            B::Modulo,
            &Value::Text("-2.5000000000000000000000000000000000001".into()),
            &Value::Int16(2)
        ),
        Ok(Value::Int32(-1))
    );

    assert_eq!(
        evaluator.apply_binary(B::Add, &Value::Text("bad".into()), &Value::Int16(1)),
        Err(EvalError::TypeMismatch)
    );
    assert_eq!(
        evaluator.apply_binary(
            B::Divide,
            &Value::Text("2".into()),
            &Value::Text("0".into())
        ),
        Err(EvalError::DivisionByZero)
    );
    assert_eq!(
        evaluator.apply_binary(
            B::Add,
            &Value::Int64(9007199254740993),
            &Value::Text("1".into())
        ),
        Ok(Value::Double(Double::from_float(9007199254740992.0)))
    );
    assert_eq!(
        evaluator.apply_binary(B::Concatenate, &Value::Int16(2), &Value::Text("3".into())),
        Ok(Value::Text("23".into()))
    );
}

#[test]
fn both_numeric_text_operands_share_limits_and_all_conversion_checks_can_cancel() {
    let left = Value::Text("2.5".into());
    let right = Value::Text("1.25".into());
    let mut budget = ResourceBudget::new(4, 0, 65536);
    assert!(
        matches!(Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .with_number_format(NumberFormat::EnUs).apply_binary(B::Multiply, &left, &right),
        Err(EvalError::Limit(error)) if error.resource == Resource::Bytes)
    );
    let mut budget = ResourceBudget::new(65536, 0, 65536);
    assert_eq!(
        Evaluator::new(
            &NoBindings,
            &mut budget,
            EvalLimits {
                max_text_bytes: 2,
                ..Default::default()
            }
        )
        .with_number_format(NumberFormat::EnUs)
        .apply_unary(U::Minus, left.clone()),
        Err(EvalError::TextLimit)
    );
    let calls = Cell::new(0);
    let invoke = |stop| {
        let cancel = || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        };
        let mut budget = ResourceBudget::new(65536, 0, 65536);
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_number_format(NumberFormat::EnUs)
            .with_cancellation(&cancel)
            .apply_binary(B::Multiply, &left, &right)
    };
    assert_eq!(invoke(usize::MAX), Ok(double(3.125)));
    let total = calls.get();
    for stop in 1..=total {
        calls.set(0);
        assert_eq!(invoke(stop), Err(EvalError::Cancelled), "checkpoint {stop}");
    }
}

proptest! {
    #![proptest_config(crate::test_support::config())]
    #[test]
    fn small_integer_text_arithmetic_matches_an_independent_wide_integer_model(a in -10000_i32..10000, b in -10000_i32..10000) {
        let mut budget = ResourceBudget::new(65536, 0, 65536);
        let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_number_format(NumberFormat::Invariant);
        for (operator, result) in [(B::Add, i64::from(a) + i64::from(b)), (B::Subtract, i64::from(a) - i64::from(b)), (B::Multiply, i64::from(a) * i64::from(b))] {
            let left = Value::Text(a.to_string());
            let right = Value::Int32(b);
            prop_assert_eq!(evaluator.apply_binary(operator, &left, &right), Ok(double(result as f64)));
        }
    }
}

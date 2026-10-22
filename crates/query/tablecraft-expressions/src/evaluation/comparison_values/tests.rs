use super::*;
use crate::{
    evaluation::{EvalLimits, NoBindings},
    functions::conversions::text::NumberFormat,
};
use std::cell::Cell;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{Currency, Decimal, Double},
};

#[test]
fn numeric_text_and_fixed_float_predicates_use_checked_conversion_targets() {
    let mut budget = ResourceBudget::new(65536, 0, 65536);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .with_number_format(NumberFormat::DeDe);
    for (operator, left, right, expected) in [
        (
            Comparison::Equal,
            Value::Int32(2),
            Value::Text("2,0".into()),
            Truth::True,
        ),
        (
            Comparison::Greater,
            Value::Text("2,5".into()),
            Value::Int32(2),
            Truth::True,
        ),
        (
            Comparison::Equal,
            Value::Int32(2),
            Value::Text("1,5".into()),
            Truth::False,
        ),
        (
            Comparison::Equal,
            Value::Currency(Currency::from_scaled(1)),
            Value::Text("0,00015".into()),
            Truth::False,
        ),
        (
            Comparison::Equal,
            Value::Currency(Currency::from_scaled(2)),
            Value::Text("0,00015".into()),
            Truth::True,
        ),
        (
            Comparison::Equal,
            Value::Currency(Currency::from_scaled(2)),
            Value::Double(Double::from_float(0.00015)),
            Truth::False,
        ),
        (
            Comparison::Equal,
            Value::Decimal(Decimal::new(1, 1).unwrap()),
            Value::Double(Double::from_float(0.1)),
            Truth::True,
        ),
        (
            Comparison::Equal,
            Value::Decimal(Decimal::new(9007199254740993125, 3).unwrap()),
            Value::Text("9007199254740993,125".into()),
            Truth::True,
        ),
    ] {
        assert_eq!(
            evaluator.compare_values(operator, &left, &right),
            Ok(expected)
        );
    }
    assert_eq!(
        evaluator.compare_values(
            Comparison::Equal,
            &Value::Int64(9007199254740993),
            &Value::Text("9007199254740993".into())
        ),
        Err(EvalError::PrecisionLoss)
    );
}

#[test]
fn strict_keys_text_text_null_and_required_policy_remain_separate() {
    let mut budget = ResourceBudget::new(65536, 0, 65536);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    assert_eq!(
        evaluator.compare_values(
            Comparison::Equal,
            &Value::Int32(2),
            &Value::Text("2".into())
        ),
        Err(EvalError::NumberFormatRequired)
    );
    assert_eq!(
        evaluator.compare_values(Comparison::Equal, &Value::Null, &Value::Text("bad".into())),
        Ok(Truth::Unknown)
    );
    let mut evaluator = evaluator.with_number_format(NumberFormat::EnUs);
    assert_eq!(
        evaluator.compare_values(
            Comparison::Equal,
            &Value::Text("2".into()),
            &Value::Text("2".into())
        ),
        Err(EvalError::CollationRequired)
    );
    assert_eq!(
        evaluator.compare_values(
            Comparison::Equal,
            &Value::Int32(2),
            &Value::Text("bad".into())
        ),
        Err(EvalError::TypeMismatch)
    );
    assert_eq!(
        comparison::compare(
            Comparison::Equal,
            &Value::Int32(2),
            &Value::Text("2".into()),
            None
        ),
        Err(EvalError::TypeMismatch)
    );
    assert_eq!(
        evaluator.compare_values(
            Comparison::Equal,
            &Value::Decimal(Decimal::new(0, 0).unwrap()),
            &Value::Double(Double::from_float(f64::MAX))
        ),
        Err(EvalError::Overflow)
    );
}

#[test]
fn conversion_limits_and_every_observed_cancellation_checkpoint_are_enforced() {
    let mut budget = ResourceBudget::new(2, 0, 65536);
    assert!(
        matches!(Evaluator::new(&NoBindings,&mut budget,EvalLimits::default()).with_number_format(NumberFormat::EnUs).compare_values(Comparison::Equal,&Value::Int32(2),&Value::Text("2.00".into())),Err(EvalError::Limit(error)) if error.resource==Resource::Bytes)
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
            .compare_values(
                Comparison::Equal,
                &Value::Int32(2),
                &Value::Text("2.00".into()),
            )
    };
    assert_eq!(invoke(usize::MAX), Ok(Truth::True));
    let total = calls.get();
    for stop in 1..=total {
        calls.set(0);
        assert_eq!(invoke(stop), Err(EvalError::Cancelled), "checkpoint {stop}");
    }
}

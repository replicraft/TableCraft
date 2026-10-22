use super::*;
use crate::{
    evaluation::{EvalLimits, NoBindings},
    functions::conversions::text::NumberFormat,
};
use std::cell::Cell;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{Double, Single},
};

#[test]
fn rounded_double_predicates_keep_single_widening_and_exact_integral_pairs_separate() {
    let mut budget = ResourceBudget::new(65536, 0, 65536);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .with_number_format(NumberFormat::EnUs);
    for (left, right, expected) in [
        (
            Value::Int64(9007199254740993),
            Value::Double(Double::from_float(9007199254740992.0)),
            Truth::True,
        ),
        (
            Value::Int64(i64::MAX),
            Value::Double(Double::from_float(9223372036854775808.0)),
            Truth::True,
        ),
        (
            Value::Int64(-9007199254740993),
            Value::Text("-9007199254740993".into()),
            Truth::True,
        ),
        (
            Value::Int64(9007199254740993),
            Value::Int64(9007199254740992),
            Truth::False,
        ),
        (
            Value::Int64(9007199254740995),
            Value::Single(Single::from_float(9007199254740992.0)),
            Truth::False,
        ),
    ] {
        assert_eq!(
            evaluator.compare_values(Comparison::Equal, &left, &right),
            Ok(expected)
        );
        assert_eq!(
            evaluator.compare_values(Comparison::Equal, &right, &left),
            Ok(expected)
        );
    }
    assert_eq!(
        evaluator.compare_values(
            Comparison::Greater,
            &Value::Int64(9007199254740995),
            &Value::Single(Single::from_float(9007199254740992.0))
        ),
        Ok(Truth::True)
    );
    assert_eq!(
        comparison::compare(
            Comparison::Equal,
            &Value::Int64(9007199254740993),
            &Value::Double(Double::from_float(9007199254740992.0)),
            None
        ),
        Err(EvalError::PrecisionLoss)
    );
    assert_eq!(
        evaluator.compare_values(
            Comparison::Equal,
            &Value::Int64(i64::MAX),
            &Value::Double(Double::from_float(f64::NAN))
        ),
        Err(EvalError::NonFinite)
    );
    assert_eq!(
        evaluator.compare_values(Comparison::Equal, &Value::Null, &Value::Text("bad".into())),
        Ok(Truth::Unknown)
    );
}

#[test]
fn rounded_predicate_conversions_share_work_cancellation_and_required_number_policy() {
    let mut budget = ResourceBudget::new(65536, 0, 65536);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default()).compare_values(
            Comparison::Equal,
            &Value::Int64(9007199254740993),
            &Value::Text("9007199254740993".into())
        ),
        Err(EvalError::NumberFormatRequired)
    );
    for (resource, mut budget) in [
        (Resource::Bytes, ResourceBudget::new(0, 0, 65536)),
        (Resource::Steps, ResourceBudget::new(65536, 0, 0)),
    ] {
        let result = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_number_format(NumberFormat::EnUs)
            .compare_values(
                Comparison::Equal,
                &Value::Int64(9007199254740993),
                &Value::Text("9007199254740993".into()),
            );
        assert!(matches!(result, Err(EvalError::Limit(error)) if error.resource == resource));
    }
    let calls = Cell::new(0);
    let invoke = |stop| {
        calls.set(0);
        let cancelled = || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        };
        let mut budget = ResourceBudget::new(65536, 0, 65536);
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_number_format(NumberFormat::EnUs)
            .with_cancellation(&cancelled)
            .compare_values(
                Comparison::Equal,
                &Value::Int64(9007199254740993),
                &Value::Text("9007199254740993".into()),
            )
    };
    assert_eq!(invoke(usize::MAX), Ok(Truth::True));
    let total = calls.get();
    for stop in 1..=total {
        assert_eq!(invoke(stop), Err(EvalError::Cancelled));
    }
}

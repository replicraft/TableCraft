use super::*;
use crate::{
    comparison::{Collation, StringComparisonProfile},
    evaluation::NoBindings,
};
use proptest::prelude::*;
use std::{cell::Cell, cmp::Ordering};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{Currency, Decimal, Double, ExpressionString, LegacyDateTime, Single},
};

struct Units;

impl Collation for Units {
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        Ok(left.cmp(right))
    }

    fn string_comparison_profile(&self) -> StringComparisonProfile {
        StringComparisonProfile::Utf16
    }

    fn compare_utf16(&self, left: &[u16], right: &[u16]) -> Result<Ordering, EvalError> {
        Ok(left.cmp(right))
    }
}

fn raw(bytes: Vec<u8>) -> ExpressionValue {
    ExpressionValue::StringBytes(ExpressionString::from_owned(bytes))
}

#[test]
fn empty_matches_zero_across_numeric_and_legacy_date_carriers() {
    let mut budget = ResourceBudget::new(65536, 0, 65536);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, Default::default());
    let mut zeros = vec![ExpressionValue::Empty];
    zeros.extend(
        [
            Value::Boolean(false),
            Value::Byte(0),
            Value::Int16(0),
            Value::Int32(0),
            Value::Int64(0),
            Value::Single(Single::from_float(-0.0)),
            Value::Double(Double::from_float(-0.0)),
            Value::Currency(Currency::from_scaled(0)),
            Value::Decimal(Decimal::new(0, 28).unwrap()),
            Value::DateTime(LegacyDateTime::from_ole_days(0.0).unwrap()),
        ]
        .map(ExpressionValue::Value),
    );

    for zero in &zeros {
        for (left, right) in [
            (&ExpressionValue::Empty, zero),
            (zero, &ExpressionValue::Empty),
        ] {
            assert_eq!(
                evaluator.compare_expression_values(Comparison::Equal, left, right),
                Ok(Truth::True)
            );
        }
    }
    assert_eq!(
        evaluator.compare_expression_values(
            Comparison::Greater,
            &ExpressionValue::Empty,
            &ExpressionValue::Value(Value::Boolean(true)),
        ),
        Ok(Truth::True)
    );
}

#[test]
fn empty_text_keeps_provider_raw_units_and_odd_range_boundaries() {
    let mut budget = ResourceBudget::new(65536, 0, 65536);
    let mut evaluator =
        Evaluator::new(&NoBindings, &mut budget, Default::default()).with_collation(&Units);

    for value in [
        ExpressionValue::Value(Value::Text(String::new())),
        raw(vec![]),
    ] {
        assert_eq!(
            evaluator.compare_expression_values(Comparison::Equal, &ExpressionValue::Empty, &value),
            Ok(Truth::True)
        );
    }
    for value in [
        ExpressionValue::Value(Value::Text("0".into())),
        raw(vec![0, 0]),
        raw(vec![0, 216]),
    ] {
        assert_eq!(
            evaluator.compare_expression_values(Comparison::Less, &ExpressionValue::Empty, &value),
            Ok(Truth::True)
        );
        assert_eq!(
            evaluator.compare_expression_values(
                Comparison::Greater,
                &value,
                &ExpressionValue::Empty
            ),
            Ok(Truth::True)
        );
    }
    assert_eq!(
        evaluator.compare_expression_values(
            Comparison::Equal,
            &ExpressionValue::Empty,
            &raw(vec![0])
        ),
        Err(EvalError::UnrepresentableText)
    );

    let mut budget = ResourceBudget::new(65536, 0, 65536);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, Default::default());
    assert_eq!(
        evaluator.compare_expression_values(
            Comparison::Equal,
            &ExpressionValue::Empty,
            &ExpressionValue::Value(Value::Text(String::new())),
        ),
        Err(EvalError::CollationRequired)
    );
}

#[test]
fn null_errors_missing_and_nonfinite_do_not_become_zero_results() {
    let mut budget = ResourceBudget::new(65536, 0, 65536);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, Default::default());
    for (operand, expected) in [
        (ExpressionValue::Value(Value::Null), Ok(Truth::Unknown)),
        (ExpressionValue::Error(0), Err(EvalError::TypeMismatch)),
        (
            ExpressionValue::Missing,
            Err(EvalError::NonScalarExpression),
        ),
        (
            ExpressionValue::Value(Value::Binary(vec![])),
            Err(EvalError::NonScalarExpression),
        ),
        (
            ExpressionValue::Value(Value::Double(Double::from_float(f64::INFINITY))),
            Err(EvalError::NonFinite),
        ),
    ] {
        for (left, right) in [
            (&ExpressionValue::Empty, &operand),
            (&operand, &ExpressionValue::Empty),
        ] {
            assert_eq!(
                evaluator.compare_expression_values(Comparison::Equal, left, right),
                expected
            );
        }
    }
}

proptest! {
    #![proptest_config(crate::test_support::config())]

    #[test]
    fn empty_integral_predicates_match_an_independent_zero_model(integer in any::<i64>()) {
        let mut budget = ResourceBudget::new(65536, 0, 65536);
        let mut evaluator = Evaluator::new(&NoBindings, &mut budget, Default::default());
        let number = ExpressionValue::Value(Value::Int64(integer));

        for (operator, reversed, expected) in [
            (Comparison::Equal, Comparison::Equal, integer == 0),
            (Comparison::NotEqual, Comparison::NotEqual, integer != 0),
            (Comparison::Less, Comparison::Greater, 0 < integer),
            (Comparison::LessOrEqual, Comparison::GreaterOrEqual, 0 <= integer),
            (Comparison::Greater, Comparison::Less, 0 > integer),
            (Comparison::GreaterOrEqual, Comparison::LessOrEqual, 0 >= integer),
        ] {
            let expected = Ok(if expected { Truth::True } else { Truth::False });
            prop_assert_eq!(
                evaluator.compare_expression_values(operator, &ExpressionValue::Empty, &number),
                expected
            );
            prop_assert_eq!(
                evaluator.compare_expression_values(reversed, &number, &ExpressionValue::Empty),
                expected
            );
        }
    }
}

#[test]
fn empty_comparisons_share_work_and_raw_allocation_limits() {
    let mut budget = ResourceBudget::new(0, 0, 100);
    assert!(matches!(
        Evaluator::new(&NoBindings, &mut budget, Default::default())
            .with_collation(&Units)
            .compare_expression_values(Comparison::Less, &ExpressionValue::Empty, &raw(vec![0, 216])),
        Err(EvalError::Limit(limit)) if limit.resource == Resource::Bytes
    ));

    let mut budget = ResourceBudget::new(0, 0, 0);
    assert!(matches!(
        Evaluator::new(&NoBindings, &mut budget, Default::default())
            .compare_expression_values(Comparison::Equal, &ExpressionValue::Empty, &ExpressionValue::Empty),
        Err(EvalError::Limit(limit)) if limit.resource == Resource::Steps
    ));
}

#[test]
fn every_observed_empty_raw_comparison_checkpoint_can_cancel() {
    let source = raw(vec![0, 216]);
    let observed = Cell::new(0);
    let count = || {
        observed.set(observed.get() + 1);
        false
    };
    let mut budget = ResourceBudget::new(65536, 0, 65536);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, Default::default())
            .with_collation(&Units)
            .with_cancellation(&count)
            .compare_expression_values(Comparison::Less, &ExpressionValue::Empty, &source),
        Ok(Truth::True)
    );

    for stop in 1..=observed.get() {
        let checks = Cell::new(0);
        let cancelled = || {
            checks.set(checks.get() + 1);
            checks.get() >= stop
        };
        let mut budget = ResourceBudget::new(65536, 0, 65536);
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, Default::default())
                .with_collation(&Units)
                .with_cancellation(&cancelled)
                .compare_expression_values(Comparison::Less, &ExpressionValue::Empty, &source),
            Err(EvalError::Cancelled)
        );
    }
}

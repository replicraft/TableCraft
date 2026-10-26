use super::*;
use crate::{
    comparison::Collation,
    evaluation::{EvalLimits, NoBindings},
    functions::{conversions::text::NumberFormat, text::cstr::DateTextFormat},
    predicates::LikeMode,
};
use std::{cell::Cell, cmp::Ordering};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Currency, Decimal, Double, ExtendedDateTime, LegacyDateTime, Single},
};

struct Ordinal;

impl Collation for Ordinal {
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        Ok(left.cmp(right))
    }
}

#[test]
fn scalar_families_coerce_in_both_positions_and_modes() {
    let cases = [
        (Value::Boolean(true), "True"),
        (Value::Boolean(false), "False"),
        (Value::Byte(12), "12"),
        (Value::Int16(-12), "-12"),
        (Value::Int32(120000), "120000"),
        (Value::Int64(9007199254740993), "9007199254740993"),
        (Value::Single(Single::from_float(1.25)), "1.25"),
        (Value::Double(Double::from_float(-1.25)), "-1.25"),
        (Value::Currency(Currency::from_scaled(12500)), "1.25"),
        (Value::Decimal(Decimal::new(125, 2).unwrap()), "1.25"),
        (
            Value::DateTime(LegacyDateTime::from_ole_days(0.0).unwrap()),
            "00:00:00",
        ),
    ];
    for mode in [LikeMode::Ansi89, LikeMode::Ansi92] {
        for (value, spelling) in &cases {
            let mut budget = ResourceBudget::new(1_000_000, 0, 1_000_000);
            let mut evaluator = Evaluator::new(&NoBindings, &mut budget, Default::default())
                .with_like_mode(mode)
                .with_collation(&Ordinal)
                .with_number_format(NumberFormat::Invariant)
                .with_date_format(DateTextFormat::Iso);
            let text = Value::Text((*spelling).into());
            for (left, right) in [(value, &text), (&text, value), (value, value)] {
                assert_eq!(
                    evaluator.apply_like(left, right, false),
                    Ok(Value::Boolean(true))
                );
                assert_eq!(
                    evaluator.apply_like(left, right, true),
                    Ok(Value::Boolean(false))
                );
            }
        }
    }
}

#[test]
fn preparation_preserves_original_strings_and_marks_only_fresh_conversions() {
    let raw = ExpressionValue::StringBytes(ExpressionString::from_owned(vec![49, 0, 50, 0]));
    let number = ExpressionValue::Value(Value::Int32(12));
    let empty = ExpressionValue::Empty;
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, Default::default())
        .with_like_mode(LikeMode::Ansi89)
        .with_number_format(NumberFormat::Invariant);
    let prepared = evaluator.prepare_expression_like(&raw, &number).unwrap();
    assert!(!prepared.is_null());
    assert!(!prepared.value_converted());
    assert!(prepared.pattern_converted());
    assert_eq!(
        evaluator
            .with_collation(&Ordinal)
            .apply_prepared_like(&prepared, false),
        Ok(Value::Boolean(true))
    );

    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, Default::default())
        .with_like_mode(LikeMode::Ansi92)
        .with_collation(&Ordinal);
    assert_eq!(
        evaluator.apply_expression_like(&empty, &empty, false),
        Ok(Value::Boolean(true))
    );
}

#[test]
fn null_bypasses_conversion_and_unsupported_states_never_use_cstr_error_spelling() {
    let unsupported = [
        ExpressionValue::Error(13),
        ExpressionValue::Missing,
        ExpressionValue::Value(Value::Binary(vec![1, 2])),
        ExpressionValue::Value(Value::DateTimeExtended(
            ExtendedDateTime::new(2026, 1, 1, 0, 0, 0, 0).unwrap(),
        )),
    ];
    let null = ExpressionValue::Value(Value::Null);
    let text = ExpressionValue::Value(Value::Text("*".into()));
    for value in unsupported {
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        let mut evaluator = Evaluator::new(&NoBindings, &mut budget, Default::default())
            .with_like_mode(LikeMode::Ansi89);
        for (left, right) in [(&value, &null), (&null, &value)] {
            assert_eq!(
                evaluator.apply_expression_like(left, right, false),
                Ok(Value::Null)
            );
        }
        let expected = if matches!(value, ExpressionValue::Missing) {
            EvalError::NonScalarExpression
        } else {
            EvalError::TypeMismatch
        };
        assert!(matches!(
            evaluator.prepare_expression_like(&value, &text),
            Err(error) if error == expected
        ));
    }
}

#[test]
fn conversion_errors_precede_provider_and_pattern_work() {
    let number = ExpressionValue::Value(Value::Int32(12));
    let date = ExpressionValue::Value(Value::DateTime(LegacyDateTime::from_ole_days(1.0).unwrap()));
    let pattern = ExpressionValue::Value(Value::Text("[".into()));
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, Default::default())
        .with_like_mode(LikeMode::Ansi89);
    assert!(matches!(
        evaluator.prepare_expression_like(&number, &pattern),
        Err(EvalError::NumberFormatRequired)
    ));
    assert!(matches!(
        evaluator.prepare_expression_like(&date, &pattern),
        Err(EvalError::DateFormatRequired)
    ));
}

#[test]
fn conversion_and_matching_share_every_cancellation_checkpoint_and_quotas() {
    let value = ExpressionValue::Value(Value::Currency(Currency::from_scaled(12500)));
    let pattern = ExpressionValue::Value(Value::Text("1*".into()));
    let checks = Cell::new(0);
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let result = Evaluator::new(&NoBindings, &mut budget, Default::default())
        .with_like_mode(LikeMode::Ansi89)
        .with_number_format(NumberFormat::Invariant)
        .with_collation(&Ordinal)
        .with_cancellation(&|| {
            checks.set(checks.get() + 1);
            false
        })
        .apply_expression_like(&value, &pattern, false);
    assert_eq!(result, Ok(Value::Boolean(true)));
    for stop in 1..=checks.get() {
        let calls = Cell::new(0);
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        let result = Evaluator::new(&NoBindings, &mut budget, Default::default())
            .with_like_mode(LikeMode::Ansi89)
            .with_number_format(NumberFormat::Invariant)
            .with_collation(&Ordinal)
            .with_cancellation(&|| {
                calls.set(calls.get() + 1);
                calls.get() >= stop
            })
            .apply_expression_like(&value, &pattern, false);
        assert_eq!(result, Err(EvalError::Cancelled), "checkpoint {stop}");
    }

    let mut budget = ResourceBudget::new(0, 0, 10000);
    let result = Evaluator::new(&NoBindings, &mut budget, Default::default())
        .with_like_mode(LikeMode::Ansi89)
        .with_number_format(NumberFormat::Invariant)
        .apply_expression_like(&value, &pattern, false);
    assert!(matches!(result, Err(EvalError::Limit(_))));

    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let result = Evaluator::new(
        &NoBindings,
        &mut budget,
        EvalLimits {
            max_text_bytes: 2,
            ..Default::default()
        },
    )
    .with_like_mode(LikeMode::Ansi89)
    .with_number_format(NumberFormat::Invariant)
    .apply_expression_like(&value, &pattern, false);
    assert_eq!(result, Err(EvalError::TextLimit));
}

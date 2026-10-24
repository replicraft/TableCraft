use super::{ConditionalFunction, expression};
use crate::{EvalError, functions::iif};
use proptest::prelude::*;
use tablecraft_core::values::{Decimal, Double, ExpressionValue, LegacyDateTime, Single, Value};

fn date(serial: f64) -> Value {
    Value::DateTime(LegacyDateTime::from_ole_days(serial).unwrap())
}

fn select(
    function: ConditionalFunction,
    values: Vec<ExpressionValue>,
) -> Result<ExpressionValue, EvalError> {
    let mut selection = function.start(values.len())?;
    for value in values {
        selection.push_expression(value)?;
    }
    selection.finish_expression()
}

#[test]
fn dates_use_unrounded_truth_and_rounded_choose_indices() {
    for (serial, truth) in [
        (-0.0, false),
        (0.0, false),
        (-0.25, true),
        (0.25, true),
        (-1.25, true),
    ] {
        assert_eq!(
            iif(&date(serial), Value::Int16(1), Value::Int16(0)),
            Ok(Value::Int16(i16::from(truth)))
        );
        let result = select(
            ConditionalFunction::Switch,
            vec![
                ExpressionValue::Value(date(serial)),
                ExpressionValue::Error(2001),
                ExpressionValue::Value(Value::Boolean(true)),
                ExpressionValue::Value(Value::Int16(0)),
            ],
        )
        .unwrap();
        assert_eq!(
            result,
            if truth {
                ExpressionValue::Error(2001)
            } else {
                ExpressionValue::Value(Value::Int16(0))
            }
        );
    }
    for (serial, index) in [
        (0.25, None),
        (1.5, Some(2)),
        (2.5, Some(2)),
        (3.5, Some(4)),
        (-1.0, None),
    ] {
        let result = select(
            ConditionalFunction::Choose,
            vec![
                ExpressionValue::Value(date(serial)),
                ExpressionValue::Value(Value::Int16(1)),
                ExpressionValue::Value(Value::Int16(2)),
                ExpressionValue::Value(Value::Int16(3)),
                ExpressionValue::Value(Value::Int16(4)),
            ],
        )
        .unwrap();
        assert_eq!(
            result,
            ExpressionValue::Value(index.map_or(Value::Null, Value::Int16))
        );
    }
}

#[test]
fn expression_states_keep_truth_selection_and_storage_boundaries_distinct() {
    assert_eq!(
        expression::iif(
            &ExpressionValue::Empty,
            ExpressionValue::Error(2001),
            ExpressionValue::Value(Value::Int16(0))
        ),
        Ok(ExpressionValue::Value(Value::Int16(0)))
    );
    assert_eq!(
        select(
            ConditionalFunction::Choose,
            vec![
                ExpressionValue::Empty,
                ExpressionValue::Value(Value::Int16(1))
            ]
        ),
        Ok(ExpressionValue::Value(Value::Null))
    );
    for invalid in [
        ExpressionValue::Missing,
        ExpressionValue::Error(0),
        ExpressionValue::Error(u32::MAX),
    ] {
        assert_eq!(
            expression::iif(
                &invalid,
                ExpressionValue::Value(Value::Int16(1)),
                ExpressionValue::Value(Value::Int16(0))
            ),
            Err(EvalError::TypeMismatch)
        );
        for function in [ConditionalFunction::Choose, ConditionalFunction::Switch] {
            assert_eq!(
                select(
                    function,
                    vec![invalid.clone(), ExpressionValue::Value(Value::Int16(1))]
                ),
                Err(EvalError::TypeMismatch)
            );
        }
    }
    let mut selection = ConditionalFunction::Choose.start(2).unwrap();
    selection.push(Value::Int16(1)).unwrap();
    selection.push_expression(ExpressionValue::Empty).unwrap();
    assert_eq!(selection.finish(), Err(EvalError::NonScalarExpression));
}

#[cfg(feature = "sql")]
#[test]
fn operational_conversion_failures_are_latched_even_if_a_caller_ignores_them() {
    let mut selection = ConditionalFunction::Choose.start(2).unwrap();
    assert_eq!(
        selection.push_failure(EvalError::Cancelled),
        Err(EvalError::Cancelled)
    );
    selection.push(Value::Int16(1)).unwrap();
    assert_eq!(selection.finish(), Err(EvalError::Cancelled));

    let mut selection = ConditionalFunction::Switch.start(4).unwrap();
    selection.push_failure(EvalError::TypeMismatch).unwrap();
    selection.push(Value::Int16(1)).unwrap();
    assert_eq!(
        selection.push_failure(EvalError::Allocation),
        Err(EvalError::Allocation)
    );
    selection.push(Value::Int16(2)).unwrap();
    assert_eq!(selection.finish(), Err(EvalError::Allocation));
}

proptest! {
    #![proptest_config(crate::test_support::config())]

    #[test]
    fn integer_date_truth_matches_nonzero_serials(serial in -650_000i32..2_900_000) {
        let result = iif(&date(f64::from(serial)), Value::Boolean(true), Value::Boolean(false));
        prop_assert_eq!(result, Ok(Value::Boolean(serial != 0)));
    }
}

#[test]
fn choose_rounds_single_indices_before_selecting_a_choice() {
    for (index, expected) in [
        (0.50000001, None),
        (1.49999999, Some(2)),
        (2.50000001, Some(2)),
        (3.49999999, Some(4)),
    ] {
        for value in [
            Value::Double(Double::from_float(index)),
            Value::DateTime(LegacyDateTime::from_ole_days(index).unwrap()),
            Value::Single(Single::from_float(index as f32)),
        ] {
            let actual = select(
                ConditionalFunction::Choose,
                vec![
                    ExpressionValue::Value(value),
                    ExpressionValue::Value(Value::Int16(1)),
                    ExpressionValue::Value(Value::Int16(2)),
                    ExpressionValue::Value(Value::Int16(3)),
                    ExpressionValue::Value(Value::Int16(4)),
                ],
            )
            .unwrap();
            assert_eq!(
                actual,
                ExpressionValue::Value(expected.map_or(Value::Null, Value::Int16))
            );
        }
    }

    let mut selection = ConditionalFunction::Choose.start(3).unwrap();
    selection
        .push(Value::Decimal(Decimal::new(149999999, 8).unwrap()))
        .unwrap();
    selection.push(Value::Int16(1)).unwrap();
    selection.push(Value::Int16(2)).unwrap();
    assert_eq!(selection.finish(), Ok(Value::Int16(2)));
}

#[test]
fn choose_checks_single_range_and_keeps_representable_out_of_list_indices_null() {
    let large = ExpressionValue::Value(Value::Int32(32768));
    assert_eq!(
        super::index::select(&large, 32768),
        Err(EvalError::Overflow)
    );
    assert_eq!(super::index::select(&large, 2), Ok(None));

    for (value, expected) in [
        (1e30, Ok(Value::Null)),
        (-1e30, Ok(Value::Null)),
        (1e100, Err(EvalError::Overflow)),
        (-1e100, Err(EvalError::Overflow)),
    ] {
        let mut selection = ConditionalFunction::Choose.start(2).unwrap();
        selection
            .push(Value::Double(Double::from_float(value)))
            .unwrap();
        selection.push(Value::Int16(1)).unwrap();
        assert_eq!(selection.finish(), expected);
    }
}

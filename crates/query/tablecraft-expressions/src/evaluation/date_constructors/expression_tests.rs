use super::*;
use crate::{
    evaluation::{EvalLimits, NoBindings},
    functions::{dates::TwoDigitYearWindow, math::random::RndState},
};
use jiff::civil::date;
use std::cell::Cell;
use tablecraft_core::{limits::ResourceBudget, values::LegacyDateTime};
use tablecraft_sql::{parse_select, select::Projection};

fn expected_date(year: i16, month: i8, day: i8) -> Value {
    Value::DateTime(LegacyDateTime::from_calendar(date(year, month, day).at(0, 0, 0, 0)).unwrap())
}

#[test]
fn empty_components_follow_integer_zero_relative_normalization_and_year_policy() {
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    for (components, expected) in [
        (
            [
                ExpressionValue::Empty,
                ExpressionValue::Value(Value::Int16(1)),
                ExpressionValue::Value(Value::Int16(1)),
            ],
            expected_date(2000, 1, 1),
        ),
        (
            [
                ExpressionValue::Value(Value::Int16(2000)),
                ExpressionValue::Empty,
                ExpressionValue::Value(Value::Int16(1)),
            ],
            expected_date(1999, 12, 1),
        ),
        (
            [
                ExpressionValue::Value(Value::Int16(2000)),
                ExpressionValue::Value(Value::Int16(1)),
                ExpressionValue::Empty,
            ],
            expected_date(1999, 12, 31),
        ),
    ] {
        assert_eq!(
            evaluator.apply_expression_date_constructor(DateConstructor::DateSerial, &components),
            Ok(expected)
        );
    }
    assert_eq!(
        evaluator.apply_expression_date_constructor(
            DateConstructor::TimeSerial,
            &[
                ExpressionValue::Empty,
                ExpressionValue::Empty,
                ExpressionValue::Empty
            ]
        ),
        Ok(expected_date(1899, 12, 30))
    );
    let mut evaluator = evaluator.with_year_window(TwoDigitYearWindow::new(1999).unwrap());
    assert_eq!(
        evaluator.apply_expression_date_constructor(
            DateConstructor::DateSerial,
            &[
                ExpressionValue::Empty,
                ExpressionValue::Value(Value::Int16(1)),
                ExpressionValue::Value(Value::Int16(1))
            ]
        ),
        Ok(expected_date(1900, 1, 1))
    );
}

#[test]
fn component_data_validation_precedes_calendar_normalization_in_written_order() {
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    for (components, error) in [
        (
            [
                ExpressionValue::Error(1),
                ExpressionValue::Value(Value::Null),
                ExpressionValue::Empty,
            ],
            EvalError::TypeMismatch,
        ),
        (
            [
                ExpressionValue::Value(Value::Null),
                ExpressionValue::Error(1),
                ExpressionValue::Empty,
            ],
            EvalError::InvalidNullConversion,
        ),
        (
            [
                ExpressionValue::Value(Value::Int32(32768)),
                ExpressionValue::Error(1),
                ExpressionValue::Empty,
            ],
            EvalError::Overflow,
        ),
        (
            [
                ExpressionValue::Value(Value::Int16(9999)),
                ExpressionValue::Value(Value::Int16(13)),
                ExpressionValue::Error(u32::MAX),
            ],
            EvalError::TypeMismatch,
        ),
        (
            [
                ExpressionValue::Value(Value::Int16(2000)),
                ExpressionValue::Missing,
                ExpressionValue::Empty,
            ],
            EvalError::NonScalarExpression,
        ),
    ] {
        assert_eq!(
            evaluator.apply_expression_date_constructor(DateConstructor::DateSerial, &components),
            Err(error)
        );
    }
}

#[test]
fn date_constructor_operands_remain_eager_and_volatile_calls_execute_once() {
    for sql in [
        "SELECT DateSerial(CVErr(1),1/0,1)",
        "SELECT TimeSerial(CVErr(1),0,1/0)",
    ] {
        let parsed = parse_select(sql, Default::default()).unwrap();
        let Projection::Expression { expression, .. } = &parsed.query().projection[0] else {
            panic!("expression required")
        };
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, EvalLimits::default()).evaluate(expression),
            Err(EvalError::DivisionByZero)
        );
    }
    let parsed = parse_select("SELECT DateSerial(CVErr(1),Rnd(),1)", Default::default()).unwrap();
    let Projection::Expression { expression, .. } = &parsed.query().projection[0] else {
        panic!("expression required")
    };
    let state = RndState::default();
    let expected = RndState::default();
    expected.apply(None).unwrap();
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_random(&state)
            .evaluate(expression),
        Err(EvalError::TypeMismatch)
    );
    assert_eq!(state.apply(None), expected.apply(None));
}

#[test]
fn constructor_state_paths_share_conversion_cancellation_and_work_limits() {
    for function in [DateConstructor::DateSerial, DateConstructor::TimeSerial] {
        let values = [
            ExpressionValue::Empty,
            ExpressionValue::Value(Value::Int16(1)),
            ExpressionValue::Value(Value::Int16(1)),
        ];
        let checks = Cell::new(0);
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_cancellation(&|| {
                checks.set(checks.get() + 1);
                false
            })
            .apply_expression_date_constructor(function, &values)
            .unwrap();
        for stop in 1..=checks.get() {
            let calls = Cell::new(0);
            let mut budget = ResourceBudget::new(65536, 0, 10000);
            assert_eq!(
                Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                    .with_cancellation(&|| {
                        calls.set(calls.get() + 1);
                        calls.get() >= stop
                    })
                    .apply_expression_date_constructor(function, &values),
                Err(EvalError::Cancelled)
            );
        }
        let mut budget = ResourceBudget::new(65536, 0, 0);
        assert!(matches!(
            Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .apply_expression_date_constructor(function, &values),
            Err(EvalError::Limit(_))
        ));
    }
}

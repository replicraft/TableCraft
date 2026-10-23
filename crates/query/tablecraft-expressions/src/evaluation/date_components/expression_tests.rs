use super::*;
use crate::{
    evaluation::{EvalLimits, NoBindings},
    functions::{
        dates::{FirstDayOfWeek, parsing::DateConversion},
        math::random::RndState,
    },
};
use std::cell::Cell;
use tablecraft_core::{limits::ResourceBudget, values::LegacyDateTime};
use tablecraft_sql::{parse_select, select::Projection};

#[test]
fn date_states_distinguish_date_zero_string_extraction_and_null() {
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    for (function, expected) in [
        (DateFunction::Year, 1899),
        (DateFunction::Month, 12),
        (DateFunction::Day, 30),
        (DateFunction::Hour, 0),
        (DateFunction::Minute, 0),
        (DateFunction::Second, 0),
    ] {
        assert_eq!(
            evaluator.apply_expression_date_function(function, &ExpressionValue::Empty),
            Ok(Value::Int16(expected))
        );
        assert_eq!(
            evaluator
                .apply_expression_date_function(function, &ExpressionValue::Value(Value::Null)),
            Ok(Value::Null)
        );
        assert_eq!(
            evaluator.apply_expression_date_function(function, &ExpressionValue::Error(1)),
            Err(EvalError::TypeMismatch)
        );
        assert_eq!(
            evaluator.apply_expression_date_function(function, &ExpressionValue::Missing),
            Err(EvalError::NonScalarExpression)
        );
    }
    for function in [
        DateConversion::CDate,
        DateConversion::CVDate,
        DateConversion::DateValue,
        DateConversion::TimeValue,
    ] {
        let date_conversion = matches!(function, DateConversion::CDate | DateConversion::CVDate);
        assert_eq!(
            evaluator.apply_expression_date_conversion(function, &ExpressionValue::Empty),
            if date_conversion {
                Ok(Value::DateTime(LegacyDateTime::from_ole_days(0.0).unwrap()))
            } else {
                Err(EvalError::TypeMismatch)
            }
        );
        assert_eq!(
            evaluator.apply_expression_date_conversion(function, &ExpressionValue::Error(1)),
            Err(EvalError::TypeMismatch)
        );
        assert_eq!(
            evaluator.apply_expression_date_conversion(function, &ExpressionValue::Missing),
            Err(EvalError::NonScalarExpression)
        );
        assert_eq!(
            evaluator
                .apply_expression_date_conversion(function, &ExpressionValue::Value(Value::Null)),
            if date_conversion {
                Err(EvalError::InvalidNullConversion)
            } else {
                Ok(Value::Null)
            }
        );
    }
}

#[test]
fn weekday_policy_validation_precedes_date_state_admission() {
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    let error = ExpressionValue::Error(1);
    assert_eq!(
        evaluator.apply_expression_weekday(&ExpressionValue::Empty, None),
        Ok(Value::Int16(7))
    );
    assert_eq!(
        evaluator.apply_expression_weekday(&error, Some(&ExpressionValue::Value(Value::Int16(8)))),
        Err(EvalError::Domain)
    );
    assert_eq!(
        evaluator.apply_expression_weekday(&error, Some(&ExpressionValue::Empty)),
        Err(EvalError::CalendarPolicyRequired)
    );
    assert_eq!(
        evaluator.apply_expression_weekday(&ExpressionValue::Missing, None),
        Err(EvalError::NonScalarExpression)
    );
    assert_eq!(
        evaluator.apply_expression_weekday(&ExpressionValue::Value(Value::Null), Some(&error)),
        Err(EvalError::TypeMismatch)
    );
    let mut limited_budget = ResourceBudget::new(65536, 0, 1000);
    let mut limited = Evaluator::new(
        &NoBindings,
        &mut limited_budget,
        EvalLimits {
            max_text_bytes: 1,
            ..Default::default()
        },
    );
    assert_eq!(
        limited.apply_expression_weekday(
            &ExpressionValue::Value(Value::Text("2024-01-02".into())),
            Some(&error)
        ),
        Err(EvalError::TextLimit)
    );
    let mut evaluator = evaluator.with_first_day_of_week(FirstDayOfWeek::new(2).unwrap());
    assert_eq!(
        evaluator.apply_expression_weekday(&ExpressionValue::Empty, Some(&ExpressionValue::Empty)),
        Ok(Value::Int16(6))
    );
    assert_eq!(
        evaluator.apply_expression_weekday(&error, Some(&ExpressionValue::Empty)),
        Err(EvalError::TypeMismatch)
    );
}

#[test]
fn date_state_sql_keeps_eager_failure_and_once_only_volatile_operands() {
    for (sql, expected) in [
        ("SELECT Weekday(CVErr(1),1/0)", EvalError::DivisionByZero),
        ("SELECT Weekday(CVErr(1),8)", EvalError::Domain),
        ("SELECT Year(CVErr(1))", EvalError::TypeMismatch),
        ("SELECT CDate(CVErr(1))", EvalError::TypeMismatch),
        ("SELECT DateValue(CVErr(1))", EvalError::TypeMismatch),
    ] {
        let parsed = parse_select(sql, Default::default()).unwrap();
        let Projection::Expression { expression, .. } = &parsed.query().projection[0] else {
            panic!("expression required")
        };
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, EvalLimits::default()).evaluate(expression),
            Err(expected),
            "{sql}"
        );
    }
    let parsed = parse_select("SELECT Weekday(CVErr(1),Rnd()+1)", Default::default()).unwrap();
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
fn single_date_state_paths_obey_every_observed_cancellation_checkpoint() {
    for operation in [Operation::Weekday, Operation::Convert, Operation::Component] {
        let checks = Cell::new(0);
        let cancelled = || {
            checks.set(checks.get() + 1);
            false
        };
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_cancellation(&cancelled);
        apply(&mut evaluator, operation).unwrap();
        for stop in 1..=checks.get() {
            let calls = Cell::new(0);
            let cancelled = || {
                calls.set(calls.get() + 1);
                calls.get() >= stop
            };
            let mut budget = ResourceBudget::new(65536, 0, 10000);
            let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_cancellation(&cancelled);
            assert_eq!(apply(&mut evaluator, operation), Err(EvalError::Cancelled));
        }
    }
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let mut evaluator =
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default()).with_cancellation(&|| true);
    assert_eq!(
        evaluator.apply_expression_weekday(&ExpressionValue::Error(1), None),
        Err(EvalError::Cancelled)
    );
    assert_eq!(
        evaluator
            .apply_expression_date_conversion(DateConversion::CDate, &ExpressionValue::Error(1)),
        Err(EvalError::Cancelled)
    );
    assert_eq!(
        evaluator.apply_expression_date_function(DateFunction::Year, &ExpressionValue::Error(1)),
        Err(EvalError::Cancelled)
    );
    for operation in [Operation::Weekday, Operation::Convert, Operation::Component] {
        let mut budget = ResourceBudget::new(65536, 0, 0);
        assert!(matches!(
            apply(
                &mut Evaluator::new(&NoBindings, &mut budget, EvalLimits::default()),
                operation
            ),
            Err(EvalError::Limit(_))
        ));
    }
}

#[derive(Clone, Copy)]
enum Operation {
    Weekday,
    Convert,
    Component,
}
fn apply(evaluator: &mut Evaluator<'_>, operation: Operation) -> Result<Value, EvalError> {
    match operation {
        Operation::Weekday => evaluator.apply_expression_weekday(&ExpressionValue::Empty, None),
        Operation::Convert => evaluator
            .apply_expression_date_conversion(DateConversion::CDate, &ExpressionValue::Empty),
        Operation::Component => {
            evaluator.apply_expression_date_function(DateFunction::Year, &ExpressionValue::Empty)
        }
    }
}

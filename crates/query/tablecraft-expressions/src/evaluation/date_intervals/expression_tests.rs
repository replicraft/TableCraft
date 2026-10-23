use super::*;
use crate::{
    evaluation::{EvalLimits, NoBindings},
    functions::{
        dates::{FirstDayOfWeek, FirstWeekOfYear},
        math::random::RndState,
    },
};
use std::cell::Cell;
use tablecraft_core::{limits::ResourceBudget, values::LegacyDateTime};
use tablecraft_sql::{parse_select, select::Projection};

fn text(value: &str) -> ExpressionValue {
    ExpressionValue::Value(Value::Text(value.into()))
}
fn number(value: i32) -> ExpressionValue {
    ExpressionValue::Value(Value::Int32(value))
}

#[test]
fn interval_empty_numeric_and_date_positions_use_zero_but_week_zero_requires_policy() {
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    assert_eq!(
        evaluator.apply_expression_date_add(
            &text("d"),
            &ExpressionValue::Empty,
            &ExpressionValue::Empty
        ),
        Ok(Value::DateTime(LegacyDateTime::from_ole_days(0.0).unwrap()))
    );
    assert_eq!(
        evaluator.apply_expression_date_part(&text("yyyy"), &ExpressionValue::Empty, None, None),
        Ok(Value::Int16(1899))
    );
    assert_eq!(
        evaluator.apply_expression_date_diff(
            &text("d"),
            &ExpressionValue::Empty,
            &number(1),
            None,
            None
        ),
        Ok(Value::Int32(1))
    );
    assert_eq!(
        evaluator.apply_expression_date_part(
            &text("yyyy"),
            &ExpressionValue::Empty,
            Some(&ExpressionValue::Empty),
            None
        ),
        Err(EvalError::CalendarPolicyRequired)
    );
    let mut evaluator = evaluator
        .with_first_day_of_week(FirstDayOfWeek::new(2).unwrap())
        .with_first_week_of_year(FirstWeekOfYear::new(2).unwrap());
    assert_eq!(
        evaluator.apply_expression_date_part(
            &text("yyyy"),
            &ExpressionValue::Empty,
            Some(&ExpressionValue::Empty),
            Some(&ExpressionValue::Empty)
        ),
        Ok(Value::Int16(1899))
    );
    assert_eq!(
        evaluator.apply_expression_date_add(&ExpressionValue::Empty, &number(1), &number(1)),
        Err(EvalError::TypeMismatch)
    );
    assert_eq!(
        evaluator.apply_expression_date_add(&text("d"), &ExpressionValue::Missing, &number(1)),
        Err(EvalError::NonScalarExpression)
    );
}

#[test]
fn interval_and_week_validation_precede_date_state_admission_progressively() {
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    assert_eq!(
        evaluator.apply_expression_date_add(
            &text("bad"),
            &ExpressionValue::Error(1),
            &ExpressionValue::Error(1)
        ),
        Err(EvalError::InvalidDateInterval)
    );
    assert_eq!(
        evaluator.apply_expression_date_add(
            &text("d"),
            &ExpressionValue::Value(Value::Int64(i64::MAX)),
            &ExpressionValue::Error(1)
        ),
        Err(EvalError::Overflow)
    );
    for part in [false, true] {
        for (day, week, error) in [
            (number(8), ExpressionValue::Error(1), EvalError::Domain),
            (
                number(0),
                ExpressionValue::Error(1),
                EvalError::CalendarPolicyRequired,
            ),
            (
                number(1),
                ExpressionValue::Error(1),
                EvalError::TypeMismatch,
            ),
        ] {
            let result = if part {
                evaluator.apply_expression_date_part(
                    &text("d"),
                    &ExpressionValue::Error(1),
                    Some(&day),
                    Some(&week),
                )
            } else {
                evaluator.apply_expression_date_diff(
                    &text("d"),
                    &ExpressionValue::Error(1),
                    &number(1),
                    Some(&day),
                    Some(&week),
                )
            };
            assert_eq!(result, Err(error));
        }
    }
    assert_eq!(
        evaluator.apply_expression_date_diff(
            &text("d"),
            &number(2958466),
            &ExpressionValue::Error(1),
            None,
            None
        ),
        Err(EvalError::Overflow)
    );
    assert_eq!(
        evaluator.apply_expression_date_diff(
            &text("d"),
            &ExpressionValue::Value(Value::Null),
            &ExpressionValue::Error(1),
            None,
            None
        ),
        Err(EvalError::TypeMismatch)
    );
}

#[test]
fn interval_sql_operands_remain_eager_and_volatile_values_execute_once() {
    for sql in [
        "SELECT DateAdd(CVErr(1),1/0,0)",
        "SELECT DateDiff('d',CVErr(1),0,1/0)",
        "SELECT DatePart('yyyy',CVErr(1),1/0)",
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
    let parsed = parse_select("SELECT DateAdd(CVErr(1),Rnd(),0)", Default::default()).unwrap();
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
fn interval_state_paths_share_conversion_work_and_cancellation() {
    for operation in [Operation::Add, Operation::Part, Operation::Diff] {
        let checks = Cell::new(0);
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        let cancelled = || {
            checks.set(checks.get() + 1);
            false
        };
        let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_cancellation(&cancelled);
        apply(&mut evaluator, operation).unwrap();
        for stop in 1..=checks.get() {
            let calls = Cell::new(0);
            let mut budget = ResourceBudget::new(65536, 0, 10000);
            let cancelled = || {
                calls.set(calls.get() + 1);
                calls.get() >= stop
            };
            let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_cancellation(&cancelled);
            assert_eq!(apply(&mut evaluator, operation), Err(EvalError::Cancelled));
        }
    }
}

#[derive(Clone, Copy)]
enum Operation {
    Add,
    Part,
    Diff,
}

fn apply(evaluator: &mut Evaluator<'_>, operation: Operation) -> Result<Value, EvalError> {
    match operation {
        Operation::Add => evaluator.apply_expression_date_add(
            &text("d"),
            &ExpressionValue::Empty,
            &ExpressionValue::Empty,
        ),
        Operation::Part => {
            evaluator.apply_expression_date_part(&text("yyyy"), &ExpressionValue::Empty, None, None)
        }
        Operation::Diff => evaluator.apply_expression_date_diff(
            &text("d"),
            &ExpressionValue::Empty,
            &number(1),
            None,
            None,
        ),
    }
}

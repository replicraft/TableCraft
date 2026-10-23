use super::*;
use crate::{
    evaluation::{EvalLimits, NoBindings},
    functions::{
        dates::{FirstDayOfWeek, parsing::DateTextFormat},
        math::random::RndState,
    },
};
use std::cell::Cell;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_sql::{parse_select, select::Projection};

fn number(value: i32) -> ExpressionValue {
    ExpressionValue::Value(Value::Int32(value))
}

#[test]
fn date_and_name_states_evaluate_late_volatile_controls_once_before_admission() {
    for sql in [
        "SELECT FormatDateTime(CVErr(11),Rnd())",
        "SELECT MonthName(CVErr(11),Rnd())",
        "SELECT WeekdayName(CVErr(11),False,Rnd())",
    ] {
        let parsed = parse_select(sql, Default::default()).unwrap();
        let Projection::Expression { expression, .. } = &parsed.query().projection[0] else {
            panic!("expression required");
        };
        let state = RndState::default();
        let expected = RndState::default();
        expected.apply(None).unwrap();
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_date_format(DateTextFormat::Iso)
                .with_random(&state)
                .evaluate(expression),
            Err(EvalError::TypeMismatch)
        );
        assert_eq!(state.apply(None), expected.apply(None));
    }
}

#[test]
fn name_states_keep_ordinal_abbreviation_domain_and_first_day_order() {
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .with_date_format(DateTextFormat::Iso)
        .with_first_day_of_week(FirstDayOfWeek::new(2).unwrap());
    assert_eq!(
        evaluator.apply_expression_calendar_name(
            TextFunction::MonthName,
            &[number(1), ExpressionValue::Empty]
        ),
        Ok(Value::Text("January".into()))
    );
    assert_eq!(
        evaluator.apply_expression_calendar_name(
            TextFunction::WeekdayName,
            &[number(1), ExpressionValue::Empty, ExpressionValue::Empty]
        ),
        Ok(Value::Text("Monday".into()))
    );
    for (function, arguments, expected) in [
        (
            TextFunction::MonthName,
            vec![ExpressionValue::Empty],
            EvalError::Domain,
        ),
        (
            TextFunction::MonthName,
            vec![number(0), ExpressionValue::Error(11)],
            EvalError::TypeMismatch,
        ),
        (
            TextFunction::MonthName,
            vec![ExpressionValue::Error(11), ExpressionValue::Missing],
            EvalError::TypeMismatch,
        ),
        (
            TextFunction::MonthName,
            vec![number(1), ExpressionValue::Missing],
            EvalError::NonScalarExpression,
        ),
        (
            TextFunction::WeekdayName,
            vec![number(0), number(0), ExpressionValue::Error(11)],
            EvalError::Domain,
        ),
        (
            TextFunction::WeekdayName,
            vec![number(1), number(0), ExpressionValue::Error(11)],
            EvalError::TypeMismatch,
        ),
    ] {
        assert_eq!(
            evaluator.apply_expression_calendar_name(function, &arguments),
            Err(expected)
        );
    }
}

#[test]
fn name_states_keep_output_limits_and_every_successful_cancellation_checkpoint() {
    for function in [TextFunction::MonthName, TextFunction::WeekdayName] {
        let calls = Cell::new(0);
        let invoke = |stop, bytes| {
            calls.set(0);
            let cancelled = || {
                calls.set(calls.get() + 1);
                calls.get() == stop
            };
            let mut budget = ResourceBudget::new(bytes, 0, 1000);
            Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_date_format(DateTextFormat::Iso)
                .with_first_day_of_week(FirstDayOfWeek::new(1).unwrap())
                .with_cancellation(&cancelled)
                .apply_expression_calendar_name(function, &[number(1), ExpressionValue::Empty])
        };
        assert!(invoke(usize::MAX, 1000).is_ok());
        let total = calls.get();
        for stop in 1..=total {
            assert_eq!(invoke(stop, 1000), Err(EvalError::Cancelled));
        }
        assert!(matches!(invoke(usize::MAX, 1), Err(EvalError::Limit(_))));
    }
}

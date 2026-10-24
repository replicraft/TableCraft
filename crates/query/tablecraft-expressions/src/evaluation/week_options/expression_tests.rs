use super::*;
use crate::{
    evaluation::{EvalLimits, NoBindings},
    functions::{
        conversions::text::NumberFormat,
        dates::{FirstDayOfWeek, FirstWeekOfYear},
        math::random::RndState,
    },
};
use std::cell::Cell;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_sql::{parse_select, select::Projection};

fn number(value: i32) -> ExpressionValue {
    ExpressionValue::Value(Value::Int32(value))
}

fn text(value: &str) -> ExpressionValue {
    ExpressionValue::Value(Value::Text(value.into()))
}

#[test]
fn format_states_keep_null_week_prepass_and_progressive_week_mask_source_admission() {
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    for (arguments, expected) in [
        (
            vec![
                ExpressionValue::Error(11),
                ExpressionValue::Error(12),
                number(8),
                ExpressionValue::Value(Value::Null),
            ],
            EvalError::InvalidNullConversion,
        ),
        (
            vec![
                ExpressionValue::Error(11),
                ExpressionValue::Error(12),
                number(8),
            ],
            EvalError::Domain,
        ),
        (
            vec![
                ExpressionValue::Error(11),
                text("0"),
                ExpressionValue::Error(12),
            ],
            EvalError::TypeMismatch,
        ),
        (
            vec![
                ExpressionValue::Error(11),
                ExpressionValue::Value(Value::Null),
            ],
            EvalError::InvalidNullConversion,
        ),
        (
            vec![ExpressionValue::Missing, text("")],
            EvalError::NonScalarExpression,
        ),
        (
            vec![ExpressionValue::Error(11), text("0")],
            EvalError::TypeMismatch,
        ),
    ] {
        assert_eq!(evaluator.apply_expression_format(&arguments), Err(expected));
    }
}

#[test]
fn format_empty_targets_keep_blank_values_distinct_from_unqualified_masked_sources() {
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .with_number_format(NumberFormat::EnUs)
        .with_first_day_of_week(FirstDayOfWeek::new(1).unwrap())
        .with_first_week_of_year(FirstWeekOfYear::new(1).unwrap());
    assert_eq!(
        evaluator.apply_expression_format(&[ExpressionValue::Empty]),
        Ok(Value::Text(String::new()))
    );
    assert_eq!(
        evaluator.apply_expression_format(&[
            number(1),
            ExpressionValue::Empty,
            ExpressionValue::Empty,
            ExpressionValue::Empty
        ]),
        Ok(Value::Text("1".into()))
    );
    assert_eq!(
        evaluator.apply_expression_format(&[ExpressionValue::Empty, text("0.00")]),
        Err(EvalError::UnsupportedExpression)
    );
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default()).apply_expression_format(&[
            number(1),
            text("0"),
            ExpressionValue::Empty
        ]),
        Err(EvalError::CalendarPolicyRequired)
    );
}

#[test]
fn format_states_share_borrowed_text_budget_and_every_cancellation_checkpoint() {
    let arguments = [text("1"), text("0.00"), text("1")];
    let mut budget = ResourceBudget::new(6, 0, 10000);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_number_format(NumberFormat::EnUs)
            .apply_expression_format(&arguments),
        Ok(Value::Text("1.00".into()))
    );
    let calls = Cell::new(0);
    let invoke = |stop| {
        calls.set(0);
        let cancelled = || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        };
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_number_format(NumberFormat::EnUs)
            .with_cancellation(&cancelled)
            .apply_expression_format(&arguments)
    };
    assert_eq!(invoke(usize::MAX), Ok(Value::Text("1.00".into())));
    let total = calls.get();
    for stop in 1..=total {
        assert_eq!(invoke(stop), Err(EvalError::Cancelled));
    }
}

#[test]
fn format_sql_keeps_late_operands_eager_and_once_only() {
    let parsed = parse_select("SELECT Format(CVErr(11),'0',Rnd())", Default::default()).unwrap();
    let Projection::Expression { expression, .. } = &parsed.query().projection[0] else {
        panic!("expression required");
    };
    let state = RndState::default();
    let expected = RndState::default();
    expected.apply(None).unwrap();
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_first_day_of_week(FirstDayOfWeek::new(1).unwrap())
            .with_random(&state)
            .evaluate(expression),
        Err(EvalError::TypeMismatch)
    );
    assert_eq!(state.apply(None), expected.apply(None));
}

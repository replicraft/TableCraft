use super::*;
use crate::{
    evaluation::{EvalLimits, NoBindings},
    functions::math::random::RndState,
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
fn slice_empty_sources_and_controls_use_their_distinct_targets() {
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    for function in [TextFunction::Left, TextFunction::Right] {
        assert_eq!(
            evaluator.apply_expression_slice(function, &[ExpressionValue::Empty, number(2)]),
            Ok(Value::Text(String::new()))
        );
        assert_eq!(
            evaluator.apply_expression_slice(function, &[text("abc"), ExpressionValue::Empty]),
            Ok(Value::Text(String::new()))
        );
    }
    assert_eq!(
        evaluator.apply_expression_slice(TextFunction::Mid, &[ExpressionValue::Empty, number(1)]),
        Ok(Value::Text(String::new()))
    );
    assert_eq!(
        evaluator.apply_expression_slice(
            TextFunction::Mid,
            &[text("abc"), number(1), ExpressionValue::Empty]
        ),
        Ok(Value::Text(String::new()))
    );
    assert_eq!(
        evaluator.apply_expression_slice(TextFunction::Mid, &[text("abc"), ExpressionValue::Empty]),
        Err(EvalError::Domain)
    );
    assert_eq!(
        evaluator
            .apply_expression_slice(TextFunction::Left, &[ExpressionValue::Missing, number(1)]),
        Err(EvalError::NonScalarExpression)
    );
    assert_eq!(
        evaluator.apply_expression_slice(
            TextFunction::Mid,
            &[text("abc"), number(1), ExpressionValue::Missing]
        ),
        Err(EvalError::NonScalarExpression)
    );
}

#[test]
fn slice_state_admission_keeps_null_source_and_progressive_conversion_order() {
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    let error = ExpressionValue::Error(11);
    for function in [TextFunction::Left, TextFunction::Right, TextFunction::Mid] {
        assert_eq!(
            evaluator.apply_expression_slice(
                function,
                &[ExpressionValue::Value(Value::Null), error.clone()]
            ),
            Ok(Value::Null)
        );
        assert_eq!(
            evaluator.apply_expression_slice(function, &[error.clone(), number(1)]),
            Err(EvalError::TypeMismatch)
        );
        assert_eq!(
            evaluator.apply_expression_slice(function, &[number(123), error.clone()]),
            Err(EvalError::NumberFormatRequired)
        );
    }
    assert_eq!(
        evaluator
            .apply_expression_slice(TextFunction::Mid, &[text("abc"), number(0), error.clone()]),
        Err(EvalError::Domain)
    );
    assert_eq!(
        evaluator
            .apply_expression_slice(TextFunction::Mid, &[text("abc"), number(1), error.clone()]),
        Err(EvalError::TypeMismatch)
    );
    let mut limited = Evaluator::new(
        &NoBindings,
        &mut budget,
        EvalLimits {
            max_text_bytes: 1,
            ..Default::default()
        },
    );
    assert_eq!(
        limited.apply_expression_slice(TextFunction::Left, &[error, text("123")]),
        Err(EvalError::TextLimit)
    );
}

#[test]
fn scalar_slice_states_keep_later_failures_eager_and_volatile_operands_once_only() {
    for function in ["Left", "Right", "Mid"] {
        let parsed = parse_select(
            &format!("SELECT {function}(CVErr(1),1/0)"),
            Default::default(),
        )
        .unwrap();
        let Projection::Expression { expression, .. } = &parsed.query().projection[0] else {
            panic!("expression required")
        };
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, EvalLimits::default()).evaluate(expression),
            Err(EvalError::DivisionByZero)
        );
        let parsed = parse_select(
            &format!("SELECT {function}(CVErr(1),Rnd())"),
            Default::default(),
        )
        .unwrap();
        let Projection::Expression { expression, .. } = &parsed.query().projection[0] else {
            panic!("expression required")
        };
        let state = RndState::default();
        let expected = RndState::default();
        expected.apply(None).unwrap();
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_random(&state)
                .evaluate(expression),
            Err(EvalError::TypeMismatch)
        );
        assert_eq!(state.apply(None), expected.apply(None));
    }
}

#[test]
fn slice_state_paths_obey_all_observed_cancellation_checkpoints_and_work_limits() {
    for function in [TextFunction::Left, TextFunction::Right, TextFunction::Mid] {
        let arguments = if function == TextFunction::Mid {
            vec![ExpressionValue::Empty, number(1), ExpressionValue::Empty]
        } else {
            vec![ExpressionValue::Empty, ExpressionValue::Empty]
        };
        let checks = Cell::new(0);
        let cancelled = || {
            checks.set(checks.get() + 1);
            false
        };
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_cancellation(&cancelled)
                .apply_expression_slice(function, &arguments),
            Ok(Value::Text(String::new()))
        );
        for stop in 1..=checks.get() {
            let calls = Cell::new(0);
            let cancelled = || {
                calls.set(calls.get() + 1);
                calls.get() >= stop
            };
            let mut budget = ResourceBudget::new(65536, 0, 10000);
            assert_eq!(
                Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                    .with_cancellation(&cancelled)
                    .apply_expression_slice(function, &arguments),
                Err(EvalError::Cancelled)
            );
        }
        let mut budget = ResourceBudget::new(65536, 0, 0);
        assert!(matches!(
            Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .apply_expression_slice(function, &arguments),
            Err(EvalError::Limit(_))
        ));
    }
}

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
fn search_empty_targets_distinguish_strings_start_and_compare() {
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());

    assert_eq!(
        evaluator.apply_expression_search(
            TextFunction::InStr,
            &[ExpressionValue::Empty, ExpressionValue::Empty]
        ),
        Ok(Value::Int32(0))
    );
    assert_eq!(
        evaluator.apply_expression_search(
            TextFunction::InStr,
            &[
                number(1),
                text("abc"),
                ExpressionValue::Empty,
                ExpressionValue::Empty
            ]
        ),
        Ok(Value::Int32(1))
    );
    assert_eq!(
        evaluator.apply_expression_search(
            TextFunction::InStrRev,
            &[text("abc"), ExpressionValue::Empty]
        ),
        Ok(Value::Int32(3))
    );
    assert_eq!(
        evaluator.apply_expression_search(
            TextFunction::InStrRev,
            &[text("abc"), text("b"), ExpressionValue::Empty]
        ),
        Err(EvalError::Domain)
    );
    assert_eq!(
        evaluator.apply_expression_search(
            TextFunction::InStr,
            &[ExpressionValue::Empty, text("abc"), text("b")]
        ),
        Err(EvalError::Domain)
    );
    assert_eq!(
        evaluator.apply_expression_search(
            TextFunction::InStrRev,
            &[text("abc"), ExpressionValue::Missing]
        ),
        Err(EvalError::NonScalarExpression)
    );
}

#[test]
fn search_states_preserve_mode_start_null_and_progressive_source_order() {
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    let error = ExpressionValue::Error(11);
    let null = ExpressionValue::Value(Value::Null);

    for (arguments, expected) in [
        (
            vec![error.clone(), text("a"), null.clone(), number(-2)],
            Err(EvalError::Domain),
        ),
        (
            vec![error.clone(), text("a"), number(0)],
            Err(EvalError::Domain),
        ),
        (vec![null.clone(), error.clone()], Ok(Value::Null)),
        (vec![error.clone(), null], Ok(Value::Null)),
        (
            vec![number(123), error.clone()],
            Err(EvalError::NumberFormatRequired),
        ),
        (vec![text("abc"), error], Err(EvalError::TypeMismatch)),
    ] {
        assert_eq!(
            evaluator.apply_expression_search(TextFunction::InStrRev, &arguments),
            expected
        );
    }
}

#[test]
fn search_state_sql_keeps_later_operational_and_volatile_arguments_eager() {
    for function in ["InStr", "InStrRev"] {
        let parsed = parse_select(
            &format!("SELECT {function}(CVErr(1),1/0)"),
            Default::default(),
        )
        .unwrap();
        let Projection::Expression { expression, .. } = &parsed.query().projection[0] else {
            panic!("expression required");
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
            panic!("expression required");
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
fn state_searches_share_every_observed_scan_cancellation_and_work_checkpoint() {
    for (function, arguments) in [
        (
            TextFunction::InStr,
            vec![
                number(1),
                text("banana"),
                text("an"),
                ExpressionValue::Empty,
            ],
        ),
        (TextFunction::InStrRev, vec![text("banana"), text("an")]),
    ] {
        let checks = Cell::new(0);
        let cancelled = || {
            checks.set(checks.get() + 1);
            false
        };
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_cancellation(&cancelled)
            .apply_expression_search(function, &arguments)
            .unwrap();

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
                    .apply_expression_search(function, &arguments),
                Err(EvalError::Cancelled)
            );
        }

        let mut budget = ResourceBudget::new(65536, 0, 0);
        assert!(matches!(
            Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .apply_expression_search(function, &arguments),
            Err(EvalError::Limit(_))
        ));
    }
}

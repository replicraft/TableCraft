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
fn replacement_empty_targets_preserve_omission_and_required_string_validation() {
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());

    for (arguments, expected) in [
        (
            vec![ExpressionValue::Empty, text("a"), text("X")],
            Ok(Value::Text(String::new())),
        ),
        (
            vec![text("banana"), text("a"), ExpressionValue::Empty],
            Ok(Value::Text("bnn".into())),
        ),
        (
            vec![
                text("banana"),
                text("a"),
                text("X"),
                number(1),
                ExpressionValue::Empty,
            ],
            Ok(Value::Text("banana".into())),
        ),
        (
            vec![text("banana"), text("a"), text("X"), ExpressionValue::Empty],
            Err(EvalError::Domain),
        ),
        (
            vec![
                text("banana"),
                text("a"),
                text("X"),
                number(1),
                number(-1),
                ExpressionValue::Empty,
            ],
            Ok(Value::Text("bXnXnX".into())),
        ),
        (
            vec![text(""), ExpressionValue::Error(11), text("X")],
            Err(EvalError::TypeMismatch),
        ),
        (
            vec![text("a"), text("a"), ExpressionValue::Missing],
            Err(EvalError::NonScalarExpression),
        ),
    ] {
        assert_eq!(evaluator.apply_expression_replacement(&arguments), expected);
    }
}

#[test]
fn replacement_state_admission_retains_control_and_source_conversion_order() {
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    let error = ExpressionValue::Error(11);

    for (arguments, expected) in [
        (
            vec![error.clone(), text("a"), text("X"), number(0)],
            EvalError::Domain,
        ),
        (
            vec![text("a"), text("a"), text("X"), number(0), error.clone()],
            EvalError::TypeMismatch,
        ),
        (
            vec![
                ExpressionValue::Value(Value::Null),
                error.clone(),
                text("X"),
            ],
            EvalError::InvalidNullConversion,
        ),
        (
            vec![number(123), error.clone(), text("X")],
            EvalError::NumberFormatRequired,
        ),
        (
            vec![
                error.clone(),
                text("a"),
                text("X"),
                number(1),
                number(-1),
                number(3),
            ],
            EvalError::Domain,
        ),
        (
            vec![
                error.clone(),
                text("a"),
                text("X"),
                number(1),
                number(-1),
                ExpressionValue::Missing,
            ],
            EvalError::NonScalarExpression,
        ),
    ] {
        assert_eq!(
            evaluator.apply_expression_replacement(&arguments),
            Err(expected)
        );
    }
}

#[test]
fn replacement_states_keep_later_operational_and_volatile_arguments_eager() {
    let parsed = parse_select("SELECT Replace(CVErr(1),'a',1/0)", Default::default()).unwrap();
    let Projection::Expression { expression, .. } = &parsed.query().projection[0] else {
        panic!("expression required");
    };
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default()).evaluate(expression),
        Err(EvalError::DivisionByZero)
    );

    let parsed = parse_select("SELECT Replace(CVErr(1),'a',Rnd())", Default::default()).unwrap();
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

#[test]
fn replacement_state_scans_and_output_share_work_bytes_and_cancellation() {
    let arguments = [text("banana"), text("a"), ExpressionValue::Empty];
    let checks = Cell::new(0);
    let cancelled = || {
        checks.set(checks.get() + 1);
        false
    };
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_cancellation(&cancelled)
            .apply_expression_replacement(&arguments),
        Ok(Value::Text("bnn".into()))
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
                .apply_expression_replacement(&arguments),
            Err(EvalError::Cancelled)
        );
    }

    for (bytes, steps) in [(2, 10000), (65536, 0)] {
        let mut budget = ResourceBudget::new(bytes, 0, steps);
        assert!(matches!(
            Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .apply_expression_replacement(&arguments),
            Err(EvalError::Limit(_))
        ));
    }
}

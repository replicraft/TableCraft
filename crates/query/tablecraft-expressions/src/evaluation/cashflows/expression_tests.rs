use super::*;
use crate::{evaluation::EvalLimits, functions::math::random::RndState};
use std::cell::Cell;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_sql::{parse_select, select::Projection};

#[test]
fn empty_scalar_controls_are_zero_but_array_states_never_become_cashflows() {
    let values = [-100, 0, 121].map(|value| ExpressionValue::Value(Value::Int16(value)));
    let mut budget = ResourceBudget::new(65536, 0, 100000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    for (function, controls, expected) in [
        (CashFlowFunction::NPV, vec![ExpressionValue::Empty], 21.0),
        (CashFlowFunction::IRR, vec![ExpressionValue::Empty], 0.1),
        (
            CashFlowFunction::MIRR,
            vec![ExpressionValue::Empty, ExpressionValue::Empty],
            0.1,
        ),
    ] {
        let Value::Double(actual) = evaluator
            .apply_expression_cashflows(function, &controls, &values)
            .unwrap()
        else {
            panic!("Double expected")
        };
        assert!((actual.to_float() - expected).abs() < 1e-10);
    }
    for value in [
        ExpressionValue::Empty,
        ExpressionValue::Error(0),
        ExpressionValue::Error(u32::MAX),
    ] {
        assert_eq!(
            evaluator.apply_expression_cashflows(
                CashFlowFunction::NPV,
                &[ExpressionValue::Empty],
                &[value]
            ),
            Err(EvalError::TypeMismatch)
        );
    }
    assert_eq!(
        evaluator.apply_expression_cashflows(
            CashFlowFunction::NPV,
            &[ExpressionValue::Empty],
            &[ExpressionValue::Missing]
        ),
        Err(EvalError::NonScalarExpression)
    );
}

#[test]
fn cashflow_admission_retains_array_and_control_conversion_order() {
    let mut budget = ResourceBudget::new(65536, 0, 100000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    for (function, controls, values, error) in [
        (
            CashFlowFunction::NPV,
            vec![ExpressionValue::Error(1)],
            vec![ExpressionValue::Value(Value::Null)],
            EvalError::TypeMismatch,
        ),
        (
            CashFlowFunction::IRR,
            vec![ExpressionValue::Error(1)],
            vec![ExpressionValue::Value(Value::Null)],
            EvalError::InvalidNullConversion,
        ),
        (
            CashFlowFunction::MIRR,
            vec![ExpressionValue::Error(1), ExpressionValue::Empty],
            vec![ExpressionValue::Value(Value::Null)],
            EvalError::InvalidNullConversion,
        ),
        (
            CashFlowFunction::NPV,
            vec![ExpressionValue::Empty],
            vec![
                ExpressionValue::Error(1),
                ExpressionValue::Value(Value::Null),
            ],
            EvalError::TypeMismatch,
        ),
        (
            CashFlowFunction::NPV,
            vec![ExpressionValue::Empty],
            vec![
                ExpressionValue::Value(Value::Null),
                ExpressionValue::Error(1),
            ],
            EvalError::InvalidNullConversion,
        ),
    ] {
        assert_eq!(
            evaluator.apply_expression_cashflows(function, &controls, &values),
            Err(error)
        );
    }
}

#[test]
fn cashflow_error_data_cannot_skip_later_expressions_or_repeat_volatile_operands() {
    for sql in [
        "SELECT NPV(CVErr(1),Array(1/0))",
        "SELECT NPV(0,Array(CVErr(1),1/0))",
        "SELECT IRR(Array(CVErr(1)),1/0)",
        "SELECT MIRR(Array(CVErr(1)),1/0,0)",
    ] {
        let parsed = parse_select(sql, Default::default()).unwrap();
        let Projection::Expression { expression, .. } = &parsed.query().projection[0] else {
            panic!("expression required")
        };
        let mut budget = ResourceBudget::new(65536, 0, 100000);
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, EvalLimits::default()).evaluate(expression),
            Err(EvalError::DivisionByZero)
        );
    }
    let parsed = parse_select("SELECT NPV(CVErr(1),Array(Rnd()))", Default::default()).unwrap();
    let Projection::Expression { expression, .. } = &parsed.query().projection[0] else {
        panic!("expression required")
    };
    let state = RndState::default();
    let expected = RndState::default();
    expected.apply(None).unwrap();
    let mut budget = ResourceBudget::new(65536, 0, 100000);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_random(&state)
            .evaluate(expression),
        Err(EvalError::TypeMismatch)
    );
    assert_eq!(state.apply(None), expected.apply(None));
}

#[test]
fn cashflow_state_paths_share_cancellation_and_numeric_buffer_allocation() {
    let values = [-100, 0, 121].map(|value| ExpressionValue::Value(Value::Int16(value)));
    for (function, controls) in [
        (CashFlowFunction::NPV, vec![ExpressionValue::Empty]),
        (CashFlowFunction::IRR, vec![ExpressionValue::Empty]),
        (
            CashFlowFunction::MIRR,
            vec![ExpressionValue::Empty, ExpressionValue::Empty],
        ),
    ] {
        let checks = Cell::new(0);
        let mut budget = ResourceBudget::new(65536, 0, 100000);
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_cancellation(&|| {
                checks.set(checks.get() + 1);
                false
            })
            .apply_expression_cashflows(function, &controls, &values)
            .unwrap();
        for stop in 1..=checks.get() {
            let calls = Cell::new(0);
            let mut budget = ResourceBudget::new(65536, 0, 100000);
            assert_eq!(
                Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                    .with_cancellation(&|| {
                        calls.set(calls.get() + 1);
                        calls.get() >= stop
                    })
                    .apply_expression_cashflows(function, &controls, &values),
                Err(EvalError::Cancelled)
            );
        }
    }
    let mut budget = ResourceBudget::new(7, 0, 100000);
    assert!(
        matches!(Evaluator::new(&NoBindings,&mut budget,EvalLimits::default())
        .apply_expression_cashflows(CashFlowFunction::NPV,&[ExpressionValue::Empty],&[ExpressionValue::Value(Value::Int16(1))]), Err(EvalError::Limit(error)) if error.resource==Resource::Bytes)
    );
}

use super::*;
use crate::{
    evaluation::{EvalLimits, NoBindings},
    functions::{math::random::RndState, text::TextFunction},
};
use std::cell::Cell;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_sql::{parse_select, select::Projection};

fn number(value: i32) -> ExpressionValue {
    ExpressionValue::Value(Value::Int32(value))
}

#[test]
fn strconv_states_keep_flags_locale_and_source_admission_order() {
    for (input, flags, locale, expected) in [
        (
            ExpressionValue::Error(11),
            number(-1),
            None,
            EvalError::Domain,
        ),
        (
            ExpressionValue::Error(11),
            number(0),
            Some(ExpressionValue::Error(12)),
            EvalError::TypeMismatch,
        ),
        (
            ExpressionValue::Error(11),
            number(0),
            Some(ExpressionValue::Empty),
            EvalError::Domain,
        ),
        (
            ExpressionValue::Error(11),
            number(0),
            None,
            EvalError::TypeMismatch,
        ),
        (
            ExpressionValue::Missing,
            number(0),
            None,
            EvalError::NonScalarExpression,
        ),
        (
            ExpressionValue::Value(Value::Null),
            ExpressionValue::Error(11),
            None,
            EvalError::TypeMismatch,
        ),
        (
            ExpressionValue::Value(Value::Null),
            number(-1),
            None,
            EvalError::Domain,
        ),
    ] {
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        let result = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .apply_expression_strconv(
                ExpressionByteOperand::Scalar(input),
                &flags,
                locale.as_ref(),
            );
        assert!(matches!(result, Err(error) if error == expected));
    }
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    let result = evaluator
        .apply_expression_strconv(
            ExpressionByteOperand::Scalar(ExpressionValue::Empty),
            &ExpressionValue::Empty,
            None,
        )
        .unwrap();
    assert_eq!(
        evaluator.publish_byte_operand(result),
        Ok(Value::Text(String::new()))
    );
}

#[test]
fn strconv_state_inputs_move_raw_bytes_and_obey_every_cancellation_checkpoint() {
    let raw = ByteString::from_owned(vec![0, 216, 255]).unwrap();
    let original = raw.as_bytes().as_ptr();
    let mut budget = ResourceBudget::new(0, 0, 1000);
    let result = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .apply_expression_strconv(
            ExpressionByteOperand::Bytes(raw),
            &ExpressionValue::Empty,
            None,
        )
        .unwrap();
    let ByteOperand::Bytes(raw) = result else {
        panic!("raw representation required");
    };
    assert_eq!(raw.as_bytes(), &[0, 216, 255]);
    assert_eq!(raw.as_bytes().as_ptr(), original);

    let calls = Cell::new(0);
    let invoke = |stop, bytes| {
        calls.set(0);
        let cancelled = || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        };
        let mut budget = ResourceBudget::new(bytes, 0, 10000);
        let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_cancellation(&cancelled);
        let result = evaluator.apply_expression_strconv(
            ExpressionByteOperand::Scalar(ExpressionValue::Value(Value::Text("straße".into()))),
            &number(129),
            Some(&number(1033)),
        )?;
        evaluator.observe_byte_operand(TextFunction::LenB, result)
    };
    assert_eq!(invoke(usize::MAX, 1000), Ok(Value::Int32(7)));
    let total = calls.get();
    for stop in 1..=total {
        assert_eq!(invoke(stop, 1000), Err(EvalError::Cancelled));
    }
    assert!(matches!(invoke(usize::MAX, 1), Err(EvalError::Limit(_))));
}

#[test]
fn strconv_states_keep_late_evaluation_eager_through_cvar_and_raw_sources() {
    let parsed = parse_select(
        "SELECT StrConv(CVar(CVErr(11)),-1,Rnd())",
        Default::default(),
    )
    .unwrap();
    let Projection::Expression { expression, .. } = &parsed.query().projection[0] else {
        panic!("expression required");
    };
    let state = RndState::default();
    let expected = RndState::default();
    expected.apply(None).unwrap();
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_random(&state)
            .evaluate(expression),
        Err(EvalError::Domain)
    );
    assert_eq!(state.apply(None), expected.apply(None));
    for (sql, expected) in [
        ("SELECT StrConv(CVErr(11),0,1/0)", EvalError::DivisionByZero),
        ("SELECT StrConv(ChrB(300),CVErr(11))", EvalError::Overflow),
    ] {
        let parsed = parse_select(sql, Default::default()).unwrap();
        let Projection::Expression { expression, .. } = &parsed.query().projection[0] else {
            panic!("expression required");
        };
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, EvalLimits::default()).evaluate(expression),
            Err(expected)
        );
    }
}

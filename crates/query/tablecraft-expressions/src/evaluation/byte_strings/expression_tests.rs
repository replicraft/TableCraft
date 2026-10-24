use super::*;
use crate::{
    evaluation::{EvalLimits, NoBindings},
    functions::{conversions::text::NumberFormat, math::random::RndState},
};
use std::cell::Cell;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_sql::{parse_select, select::Projection};

fn number(value: i32) -> ExpressionValue {
    ExpressionValue::Value(Value::Int32(value))
}

fn input(value: ExpressionValue) -> ExpressionByteOperand {
    ExpressionByteOperand::Scalar(value)
}

#[test]
fn byte_state_targets_keep_numeric_codes_string_sources_and_control_order_distinct() {
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    let code = evaluator
        .apply_expression_byte_string(TextFunction::ChrB, input(ExpressionValue::Empty), &[])
        .unwrap();
    let ByteOperand::Bytes(code) = code else {
        panic!("byte code required");
    };
    assert_eq!(code.as_bytes(), &[0]);
    assert_eq!(
        evaluator
            .observe_expression_byte_operand(TextFunction::LenB, input(ExpressionValue::Empty)),
        Ok(Value::Int32(0))
    );
    assert_eq!(
        evaluator
            .observe_expression_byte_operand(TextFunction::AscB, input(ExpressionValue::Empty)),
        Err(EvalError::Domain)
    );
    for function in [TextFunction::LenB, TextFunction::AscB] {
        assert_eq!(
            evaluator.observe_expression_byte_operand(function, input(ExpressionValue::Error(11))),
            Err(EvalError::TypeMismatch)
        );
        assert_eq!(
            evaluator.observe_expression_byte_operand(function, input(ExpressionValue::Missing)),
            Err(EvalError::NonScalarExpression)
        );
    }
    for function in [
        TextFunction::LeftB,
        TextFunction::RightB,
        TextFunction::MidB,
    ] {
        let result = evaluator.apply_expression_byte_string(
            function,
            input(ExpressionValue::Error(11)),
            &[number(-1)],
        );
        assert!(matches!(result, Err(EvalError::Domain)));
        let result = evaluator
            .apply_expression_byte_string(function, input(ExpressionValue::Empty), &[number(1)])
            .unwrap();
        assert_eq!(
            evaluator.observe_byte_operand(TextFunction::LenB, result),
            Ok(Value::Int32(0))
        );
    }
    let result = evaluator.apply_expression_byte_string(
        TextFunction::MidB,
        input(ExpressionValue::Error(11)),
        &[number(0), ExpressionValue::Error(12)],
    );
    assert!(matches!(result, Err(EvalError::TypeMismatch)));
}

#[test]
fn byte_search_states_keep_mode_start_null_and_progressive_source_admission() {
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    for (start, haystack, needle, mode, expected) in [
        (
            ExpressionValue::Error(11),
            ExpressionValue::Empty,
            ExpressionValue::Empty,
            number(3),
            Err(EvalError::UnsupportedExpression),
        ),
        (
            ExpressionValue::Empty,
            ExpressionValue::Value(Value::Null),
            ExpressionValue::Error(11),
            number(0),
            Err(EvalError::Domain),
        ),
        (
            number(1),
            ExpressionValue::Value(Value::Null),
            ExpressionValue::Error(11),
            number(0),
            Ok(Value::Null),
        ),
        (
            number(1),
            ExpressionValue::Missing,
            ExpressionValue::Value(Value::Null),
            number(0),
            Ok(Value::Null),
        ),
        (
            number(1),
            ExpressionValue::Empty,
            ExpressionValue::Empty,
            number(0),
            Ok(Value::Int32(0)),
        ),
        (
            number(1),
            number(123),
            ExpressionValue::Error(11),
            number(0),
            Err(EvalError::NumberFormatRequired),
        ),
    ] {
        assert_eq!(
            evaluator.apply_expression_byte_search(
                Some(&start),
                input(haystack),
                input(needle),
                Some(&mode)
            ),
            expected
        );
    }
}

#[test]
fn byte_state_views_move_without_copying_and_all_work_can_cancel() {
    let raw = ByteString::from_owned(vec![0, 216, 255]).unwrap();
    let original = raw.as_bytes().as_ptr();
    let mut budget = ResourceBudget::new(0, 0, 1000);
    let result = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .apply_expression_byte_string(
            TextFunction::MidB,
            ExpressionByteOperand::Bytes(raw),
            &[number(2), number(1)],
        )
        .unwrap();
    let ByteOperand::Bytes(raw) = result else {
        panic!("raw view required");
    };
    assert_eq!(raw.as_bytes(), &[216]);
    assert_eq!(raw.as_bytes().as_ptr(), original.wrapping_add(1));

    for function in [
        TextFunction::LeftB,
        TextFunction::RightB,
        TextFunction::MidB,
    ] {
        let calls = Cell::new(0);
        let invoke = |stop| {
            calls.set(0);
            let cancelled = || {
                calls.set(calls.get() + 1);
                calls.get() == stop
            };
            let mut budget = ResourceBudget::new(65536, 0, 10000);
            let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_number_format(NumberFormat::EnUs)
                .with_cancellation(&cancelled);
            let result = evaluator.apply_expression_byte_string(
                function,
                input(ExpressionValue::Value(Value::Text("abc".into()))),
                &[ExpressionValue::Value(Value::Text("3".into()))],
            )?;
            evaluator.observe_byte_operand(TextFunction::LenB, result)
        };
        assert!(invoke(usize::MAX).is_ok());
        let total = calls.get();
        for stop in 1..=total {
            assert_eq!(invoke(stop), Err(EvalError::Cancelled));
        }
    }
    let calls = Cell::new(0);
    let invoke = |stop| {
        calls.set(0);
        let cancelled = || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        };
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_cancellation(&cancelled)
            .apply_expression_byte_search(
                Some(&number(1)),
                ExpressionByteOperand::Bytes(ByteString::from_owned(vec![97, 98, 98]).unwrap()),
                ExpressionByteOperand::Bytes(ByteString::from_owned(vec![98]).unwrap()),
                Some(&number(0)),
            )
    };
    assert_eq!(invoke(usize::MAX), Ok(Value::Int32(2)));
    let total = calls.get();
    for stop in 1..=total {
        assert_eq!(invoke(stop), Err(EvalError::Cancelled));
    }
}

#[test]
fn byte_state_calls_keep_late_operational_and_volatile_operands_eager() {
    for sql in [
        "SELECT LeftB(CVar(CVErr(11)),Rnd())",
        "SELECT InStrB(1,CVErr(11),ChrB(1),Rnd())",
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
                .with_random(&state)
                .evaluate(expression),
            Err(EvalError::TypeMismatch)
        );
        assert_eq!(state.apply(None), expected.apply(None));
    }
}

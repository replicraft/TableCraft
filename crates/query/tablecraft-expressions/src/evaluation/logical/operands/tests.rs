use super::*;
use crate::{
    evaluation::NoBindings,
    functions::{conversions::text::NumberFormat, text::byte_string::ByteString},
};
use std::cell::Cell;
use tablecraft_core::{limits::ResourceBudget, values::Double};
use tablecraft_sql::{parse_expression, select::BinaryOperator};

fn apply(
    operator: Logical,
    left: ExpressionValue,
    right: ExpressionValue,
) -> Result<Value, EvalError> {
    Evaluator::new(
        &NoBindings,
        &mut ResourceBudget::new(65536, 0, 10000),
        Default::default(),
    )
    .with_number_format(NumberFormat::EnUs)
    .apply_expression_logical(operator, left, right)
}

fn raw(bytes: Vec<u8>) -> ExpressionValue {
    ExpressionValue::StringBytes(
        ByteString::from_owned(bytes)
            .unwrap()
            .into_expression_string(),
    )
}

fn bits(operator: Logical, left: i64, right: i64) -> i64 {
    match operator {
        Logical::And => left & right,
        Logical::Or => left | right,
        Logical::Xor => left ^ right,
        Logical::Equivalence => !(left ^ right),
        Logical::Implication => !left | right,
    }
}

#[test]
fn empty_logical_operands_select_integer_zero_without_becoming_boolean_or_byte() {
    for operator in [
        Logical::And,
        Logical::Or,
        Logical::Xor,
        Logical::Equivalence,
        Logical::Implication,
    ] {
        assert_eq!(
            apply(operator, ExpressionValue::Empty, ExpressionValue::Empty),
            Ok(Value::Int16(bits(operator, 0, 0) as i16))
        );
        for (value, number, width) in [
            (Value::Boolean(true), -1, 16),
            (Value::Byte(255), 255, 16),
            (Value::Int16(-32768), -32768, 16),
            (Value::Int32(i32::MIN), i64::from(i32::MIN), 32),
            (Value::Int64(9007199254740993), 9007199254740993, 64),
            (Value::Double(Double::from_float(2.5)), 2, 32),
            (Value::Text("2.5".into()), 2, 32),
        ] {
            for reversed in [false, true] {
                let (left, right, answer) = if reversed {
                    (
                        ExpressionValue::Value(value.clone()),
                        ExpressionValue::Empty,
                        bits(operator, number, 0),
                    )
                } else {
                    (
                        ExpressionValue::Empty,
                        ExpressionValue::Value(value.clone()),
                        bits(operator, 0, number),
                    )
                };
                let expected = match width {
                    16 => Value::Int16(answer as i16),
                    32 => Value::Int32(answer as i32),
                    _ => Value::Int64(answer),
                };
                assert_eq!(apply(operator, left, right), Ok(expected));
            }
        }
    }

    let result = Evaluator::new(
        &NoBindings,
        &mut ResourceBudget::new(0, 0, 100),
        Default::default(),
    )
    .apply_expression_complement(ExpressionValue::Empty);
    assert_eq!(result, Ok(Value::Int16(-1)));
}

#[test]
fn empty_and_null_use_integer_tables_and_never_suppress_peer_conversion() {
    for (operator, left, right) in [
        (Logical::And, Value::Int16(0), Value::Int16(0)),
        (Logical::Or, Value::Int16(0), Value::Int16(0)),
        (Logical::Xor, Value::Null, Value::Null),
        (Logical::Equivalence, Value::Null, Value::Null),
        (Logical::Implication, Value::Int16(-1), Value::Null),
    ] {
        assert_eq!(
            apply(
                operator,
                ExpressionValue::Empty,
                ExpressionValue::Value(Value::Null)
            ),
            Ok(left)
        );
        assert_eq!(
            apply(
                operator,
                ExpressionValue::Value(Value::Null),
                ExpressionValue::Empty
            ),
            Ok(right)
        );
    }
    for value in [Value::Null, Value::Int16(0)] {
        assert_eq!(
            apply(
                Logical::And,
                ExpressionValue::Value(value),
                raw(vec![0, 216])
            ),
            Err(EvalError::UnrepresentableText)
        );
    }
    assert_eq!(
        apply(
            Logical::And,
            ExpressionValue::Empty,
            ExpressionValue::Error(11)
        ),
        Err(EvalError::TypeMismatch)
    );
    assert_eq!(
        apply(Logical::And, raw(vec![0, 216]), ExpressionValue::Error(11)),
        Err(EvalError::TypeMismatch)
    );
    assert_eq!(
        apply(
            Logical::Or,
            ExpressionValue::Missing,
            ExpressionValue::Empty
        ),
        Err(EvalError::NonScalarExpression)
    );
}

#[test]
fn raw_strings_use_exact_numeric_text_conversion_and_shared_limits() {
    let text = "9007199254740993";
    let bytes: Vec<_> = text.encode_utf16().flat_map(u16::to_le_bytes).collect();
    assert_eq!(
        apply(
            Logical::Or,
            raw(bytes.clone()),
            ExpressionValue::Value(Value::Int64(0))
        ),
        Ok(Value::Int64(9007199254740993))
    );

    let mut budget = ResourceBudget::new(65536, 0, 10000);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, Default::default())
            .apply_expression_complement(raw(vec![50, 0])),
        Err(EvalError::NumberFormatRequired)
    );

    let mut budget = ResourceBudget::new(1, 0, 10000);
    assert!(matches!(
        Evaluator::new(&NoBindings, &mut budget, Default::default())
            .with_number_format(NumberFormat::EnUs)
            .apply_expression_complement(raw(bytes)),
        Err(EvalError::Limit(_))
    ));

    assert_eq!(
        apply(
            Logical::And,
            ExpressionValue::Value(Value::Double(Double::from_float(2147483648.0))),
            raw(vec![0, 216])
        ),
        Err(EvalError::Overflow)
    );
}

#[test]
fn every_retained_logical_checkpoint_can_cancel_including_empty_only_inputs() {
    let count = Cell::new(0);
    let observe = || {
        count.set(count.get() + 1);
        false
    };
    let run = |cancelled: &dyn Fn() -> bool| {
        Evaluator::new(
            &NoBindings,
            &mut ResourceBudget::new(65536, 0, 10000),
            Default::default(),
        )
        .with_number_format(NumberFormat::EnUs)
        .with_cancellation(cancelled)
        .apply_expression_logical(Logical::And, raw(vec![50, 0]), ExpressionValue::Empty)
    };
    assert_eq!(run(&observe), Ok(Value::Int32(0)));
    for stop in 1..=count.get() {
        let current = Cell::new(0);
        assert_eq!(
            run(&|| {
                current.set(current.get() + 1);
                current.get() == stop
            }),
            Err(EvalError::Cancelled)
        );
    }

    let mut budget = ResourceBudget::new(0, 0, 100);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, Default::default())
            .with_cancellation(&|| true)
            .apply_expression_complement(ExpressionValue::Empty),
        Err(EvalError::Cancelled)
    );
}

#[test]
fn syntax_evaluates_both_logical_operands_before_raw_publication() {
    for operator in [
        BinaryOperator::And,
        BinaryOperator::Or,
        BinaryOperator::Xor,
        BinaryOperator::Equivalence,
        BinaryOperator::Implication,
    ] {
        let name = match operator {
            BinaryOperator::And => "And",
            BinaryOperator::Or => "Or",
            BinaryOperator::Xor => "Xor",
            BinaryOperator::Equivalence => "Eqv",
            _ => "Imp",
        };
        for raw in ["ChrW(55296)", "ChrB(65)", "LeftB('😀',2)"] {
            let sql = format!("{raw} {name} (1/0)");
            let expression = parse_expression(&sql, Default::default()).unwrap();
            let result = Evaluator::new(
                &NoBindings,
                &mut ResourceBudget::new(65536, 0, 10000),
                Default::default(),
            )
            .with_number_format(NumberFormat::EnUs)
            .evaluate(&expression);
            assert_eq!(result, Err(EvalError::DivisionByZero), "{sql}");
        }
    }
}

use super::*;
use crate::{evaluation::NoBindings, functions::text::byte_string::ByteString};
use proptest::prelude::*;
use std::cell::Cell;
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Double, ExpressionValue, Guid},
};
use tablecraft_sql::parse_expression;

fn evaluate(sql: &str) -> Result<Value, EvalError> {
    let expression = parse_expression(sql, Default::default()).unwrap();
    Evaluator::new(
        &NoBindings,
        &mut ResourceBudget::new(65536, 0, 10000),
        Default::default(),
    )
    .evaluate(&expression)
}

#[test]
fn string_addition_retains_units_and_null_without_implicit_numeric_concatenation() {
    for (sql, expected) in [
        ("ChrW(55357)+ChrW(56832)", Value::Text("😀".into())),
        ("Left('😀',1)+Right('😀',1)", Value::Text("😀".into())),
        ("Len(ChrW(55296)+'x')", Value::Int32(2)),
        ("AscW(ChrW(55296)+'x')", Value::Int16(-10240)),
        ("VarType(ChrW(55296)+'')", Value::Int16(8)),
        ("ChrW(55296)+NULL", Value::Null),
        ("NULL+ChrB(65)", Value::Null),
        ("(ChrW(55296)+NULL)&'x'", Value::Text("x".into())),
        ("Mid(ChrW(55296)+'x',2)", Value::Text("x".into())),
        ("LeftB('12',4)+'34'", Value::Text("1234".into())),
    ] {
        assert_eq!(evaluate(sql), Ok(expected), "{sql}");
    }
    for (sql, expected) in [
        ("ChrW(55296)+'x'", EvalError::UnrepresentableText),
        ("LenB(ChrB(65)+ChrB(0))", EvalError::UnrepresentableText),
        ("ChrW(55296)+CVErr(1)", EvalError::TypeMismatch),
        ("CVErr(1)+ChrW(55296)", EvalError::TypeMismatch),
        ("ChrW(55296)+(1/0)", EvalError::DivisionByZero),
        ("(ChrW(55296)+NULL)+(1/0)", EvalError::DivisionByZero),
        ("LeftB('12',4)+1", EvalError::NumberFormatRequired),
    ] {
        assert_eq!(evaluate(sql), Err(expected), "{sql}");
    }

    for (value, expected) in [
        (Value::Binary(vec![0]), EvalError::TypeMismatch),
        (
            Value::Guid(Guid::from_bytes([0; 16])),
            EvalError::TypeMismatch,
        ),
        (
            Value::Double(Double::from_float(f64::INFINITY)),
            EvalError::NonFinite,
        ),
    ] {
        let raw = ByteString::from_owned(vec![0, 216]).unwrap();
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        let result = Evaluator::new(&NoBindings, &mut budget, Default::default())
            .prepare_expression_addition(
                ExpressionByteOperand::Bytes(raw),
                ExpressionByteOperand::Scalar(ExpressionValue::Value(value)),
            );
        assert!(matches!(result, Err(error) if error == expected));
    }
}

#[test]
fn string_addition_meters_buffers_and_all_observed_work_can_cancel() {
    let expression = parse_expression("Left('😀',1)+Right('😀',1)", Default::default()).unwrap();
    let mut budget = ResourceBudget::new(1, 0, 10000);
    assert!(matches!(
        Evaluator::new(&NoBindings, &mut budget, Default::default()).evaluate(&expression),
        Err(EvalError::Limit(_))
    ));

    let checkpoints = Cell::new(0);
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    Evaluator::new(&NoBindings, &mut budget, Default::default())
        .with_cancellation(&|| {
            checkpoints.set(checkpoints.get() + 1);
            false
        })
        .evaluate(&expression)
        .unwrap();

    for stop in 1..=checkpoints.get() {
        let current = Cell::new(0);
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, Default::default())
                .with_cancellation(&|| {
                    current.set(current.get() + 1);
                    current.get() == stop
                })
                .evaluate(&expression),
            Err(EvalError::Cancelled),
            "checkpoint {stop}"
        );
    }
}

proptest! {
    #![proptest_config(crate::test_support::config())]

    #[test]
    fn added_surrogate_halves_reconstruct_the_supplementary_character(code in 0x10000u32..=0x10ffff) {
        let offset = code - 0x10000;
        let high = 0xd800 + (offset >> 10);
        let low = 0xdc00 + (offset & 0x3ff);
        let sql = format!("ChrW({high})+ChrW({low})");
        prop_assert_eq!(evaluate(&sql), Ok(Value::Text(char::from_u32(code).unwrap().to_string())));
    }
}

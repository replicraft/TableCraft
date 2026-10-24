use super::*;
use crate::evaluation::NoBindings;
use proptest::prelude::*;
use std::cell::Cell;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_sql::{parse_expression, select::BinaryOperator};

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
fn concatenation_joins_surrogate_units_before_publication_and_keeps_null_rules() {
    for (sql, expected) in [
        ("ChrW(55357)&ChrW(56832)", Value::Text("😀".into())),
        ("Left('😀',1)&Right('😀',1)", Value::Text("😀".into())),
        ("Len(ChrW(55296)&NULL)", Value::Int32(1)),
        ("Len(NULL&ChrW(55296))", Value::Int32(1)),
        ("LenB(ChrW(55296)&'x')", Value::Int32(4)),
        ("AscW(ChrW(55296)&'x')", Value::Int16(-10240)),
        ("VarType(ChrW(55296)&NULL)", Value::Int16(8)),
        (
            "TypeName(CVar(ChrW(55296)&''))",
            Value::Text("String".into()),
        ),
        ("Left(ChrW(55296)&'x',0)", Value::Text(String::new())),
        ("Mid(ChrW(55296)&'x',2)", Value::Text("x".into())),
        ("NULL&NULL", Value::Null),
        ("Nz(NULL)&'x'", Value::Text("x".into())),
        ("'x'&Nz(NULL)", Value::Text("x".into())),
        ("Nz(NULL)&Nz(NULL)", Value::Text(String::new())),
        ("NULL&Nz(NULL)", Value::Text(String::new())),
    ] {
        assert_eq!(evaluate(sql), Ok(expected), "{sql}");
    }
    for (sql, expected) in [
        ("ChrW(55296)&'x'", EvalError::UnrepresentableText),
        ("LenB(ChrB(65)&ChrB(0))", EvalError::UnrepresentableText),
        ("ChrW(55296)&CVErr(1)", EvalError::TypeMismatch),
        ("CVErr(1)&ChrW(55296)", EvalError::TypeMismatch),
        ("Nz(NULL)&CVErr(1)", EvalError::TypeMismatch),
        ("ChrW(55296)&(1/0)", EvalError::DivisionByZero),
        ("Len(NULL&(1/0))", EvalError::DivisionByZero),
        ("ChrW(55296)&1", EvalError::NumberFormatRequired),
    ] {
        assert_eq!(evaluate(sql), Err(expected), "{sql}");
    }

    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, Default::default());
    assert_eq!(
        evaluator.apply_expression_binary(
            BinaryOperator::Concatenate,
            ExpressionValue::Empty,
            ExpressionValue::Value(Value::Null)
        ),
        Ok(Value::Text(String::new()))
    );
    let missing = evaluator.apply_expression_concatenation(
        ExpressionByteOperand::Scalar(ExpressionValue::Missing),
        ExpressionByteOperand::Scalar(ExpressionValue::Empty),
    );
    assert!(matches!(missing, Err(EvalError::NonScalarExpression)));
}

#[test]
fn concatenation_charges_before_allocating_and_all_observed_work_can_cancel() {
    let expression = parse_expression("Left('😀',1)&Right('😀',1)", Default::default()).unwrap();
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

    // These caller-owned input buffers are already charged. This budget covers
    // only the new join and exercises several bounded copy checkpoints.
    let inputs = || {
        [0, 1].map(|byte| {
            ExpressionByteOperand::Bytes(ByteString::from_owned(vec![byte; 8192]).unwrap())
        })
    };
    let [left, right] = inputs();
    let mut budget = ResourceBudget::new(16383, 0, 10000);
    assert!(matches!(
        Evaluator::new(&NoBindings, &mut budget, Default::default())
            .apply_expression_concatenation(left, right),
        Err(EvalError::Limit(_))
    ));

    let [left, right] = inputs();
    let checkpoints = Cell::new(0);
    let mut budget = ResourceBudget::new(16384, 0, 10000);
    let result = Evaluator::new(&NoBindings, &mut budget, Default::default())
        .with_cancellation(&|| {
            checkpoints.set(checkpoints.get() + 1);
            false
        })
        .apply_expression_concatenation(left, right)
        .unwrap();
    let ByteOperand::Bytes(result) = result else {
        panic!("raw join required");
    };
    assert_eq!(result.len(), 16384);
    assert!(result.as_bytes()[..8192].iter().all(|byte| *byte == 0));
    assert!(result.as_bytes()[8192..].iter().all(|byte| *byte == 1));

    for stop in 1..=checkpoints.get() {
        let [left, right] = inputs();
        let current = Cell::new(0);
        let mut budget = ResourceBudget::new(16384, 0, 10000);
        let result = Evaluator::new(&NoBindings, &mut budget, Default::default())
            .with_cancellation(&|| {
                current.set(current.get() + 1);
                current.get() == stop
            })
            .apply_expression_concatenation(left, right);
        assert!(
            matches!(result, Err(EvalError::Cancelled)),
            "checkpoint {stop}"
        );
    }
}

proptest! {
    #![proptest_config(crate::test_support::config())]

    #[test]
    fn joined_surrogates_publish_the_original_supplementary_character(code in 0x10000u32..=0x10ffff) {
        let offset = code - 0x10000;
        let high = 0xd800 + (offset >> 10);
        let low = 0xdc00 + (offset & 0x3ff);
        let sql = format!("ChrW({high})&ChrW({low})");
        let expected = char::from_u32(code).unwrap().to_string();
        prop_assert_eq!(evaluate(&sql), Ok(Value::Text(expected)));
    }
}

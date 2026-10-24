use crate::{
    EvalError,
    evaluation::{EvalLimits, Evaluator, NoBindings},
    functions::text::{
        TextFunction,
        byte_string::{ByteOperand, ByteString, ExpressionByteOperand},
    },
};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{ExpressionValue, Value},
};
use tablecraft_sql::parse_expression;

fn evaluate(sql: &str) -> Result<Value, EvalError> {
    let expression = parse_expression(sql, Default::default()).unwrap();
    let mut budget = ResourceBudget::new(65536, 0, 100000);
    Evaluator::new(&NoBindings, &mut budget, Default::default()).evaluate(&expression)
}

#[test]
fn trimming_preserves_units_through_observers_nested_consumers_and_repaired_output() {
    for (sql, expected) in [
        ("LenB(Trim(' '&ChrW(55296)&' '))", Value::Int32(2)),
        ("LenB(LTrim(' '&ChrW(55296)&' '))", Value::Int32(4)),
        ("LenB(RTrim(' '&ChrW(55296)&' '))", Value::Int32(4)),
        ("AscW(Trim(' '&ChrW(57343)&' '))", Value::Int16(-8193)),
        (
            "StrComp(Trim(ChrW(9)&ChrW(55296)&ChrW(160)),ChrW(9)&ChrW(55296)&ChrW(160),0)",
            Value::Int16(0),
        ),
        (
            "AscW(Trim(MidB(ChrW(8192)&ChrW(0)&ChrW(8192)&ChrW(0),2,6)))",
            Value::Int16(0),
        ),
        (
            "Trim(' '&ChrW(55357)&' ')&ChrW(56832)",
            Value::Text("😀".into()),
        ),
        (
            "LenB(Trim(Replace(' '&ChrW(55296)&' ','z','')))",
            Value::Int32(2),
        ),
        ("Trim(NULL)", Value::Null),
    ] {
        assert_eq!(evaluate(sql), Ok(expected), "{sql}");
    }
    for (sql, expected) in [
        ("Trim(ChrW(55296))", EvalError::UnrepresentableText),
        ("LenB(Trim(ChrB(32)))", EvalError::UnrepresentableText),
        ("Trim(CVErr(11))", EvalError::TypeMismatch),
        ("Trim(1)", EvalError::NumberFormatRequired),
        ("Trim(1/0)", EvalError::DivisionByZero),
    ] {
        assert_eq!(evaluate(sql), Err(expected), "{sql}");
    }
}

#[test]
fn adopted_unaligned_ranges_trim_without_copying_or_payload_budget() {
    let bytes = ByteString::from_owned(vec![9, 32, 0, 0, 216, 32, 0, 9])
        .unwrap()
        .mid(2, Some(6))
        .unwrap();
    let expected_pointer = bytes.as_bytes().as_ptr().wrapping_add(2);
    let mut budget = ResourceBudget::new(0, 0, 1000);
    let result = Evaluator::new(&NoBindings, &mut budget, Default::default())
        .apply_expression_byte_string(TextFunction::Trim, ExpressionByteOperand::Bytes(bytes), &[])
        .unwrap();
    let ByteOperand::Bytes(result) = result else {
        panic!("raw range required");
    };
    assert_eq!(result.as_bytes(), [0, 216]);
    assert_eq!(result.as_bytes().as_ptr(), expected_pointer);
}

#[test]
fn retained_trimming_admits_empty_null_error_and_missing_without_changing_scalar_limits() {
    for function in [TextFunction::LTrim, TextFunction::RTrim, TextFunction::Trim] {
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        let mut evaluator = Evaluator::new(&NoBindings, &mut budget, Default::default());
        let empty = evaluator
            .apply_expression_byte_string(
                function,
                ExpressionByteOperand::Scalar(ExpressionValue::Empty),
                &[],
            )
            .unwrap();
        assert_eq!(
            evaluator.publish_byte_operand(empty),
            Ok(Value::Text(String::new()))
        );
        let null = evaluator
            .apply_expression_byte_string(
                function,
                ExpressionByteOperand::Scalar(ExpressionValue::Value(Value::Null)),
                &[],
            )
            .unwrap();
        assert_eq!(evaluator.publish_byte_operand(null), Ok(Value::Null));
        for (value, expected) in [
            (ExpressionValue::Error(11), EvalError::TypeMismatch),
            (ExpressionValue::Missing, EvalError::NonScalarExpression),
        ] {
            assert!(
                matches!(evaluator.apply_expression_byte_string(function, ExpressionByteOperand::Scalar(value), &[]), Err(error) if error == expected)
            );
        }
    }
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    assert!(matches!(
        Evaluator::new(
            &NoBindings,
            &mut budget,
            EvalLimits {
                max_text_bytes: 1,
                ..Default::default()
            }
        )
        .apply_byte_string(
            TextFunction::Trim,
            ByteOperand::Scalar(Value::Text(" a ".into())),
            &[]
        ),
        Err(EvalError::TextLimit)
    ));
    let mut budget = ResourceBudget::new(0, 0, 0);
    assert!(matches!(
        Evaluator::new(&NoBindings, &mut budget, Default::default()).apply_byte_string(
            TextFunction::Trim,
            ByteOperand::Bytes(ByteString::from_owned(vec![32, 0]).unwrap()),
            &[]
        ),
        Err(EvalError::Limit(_))
    ));
}

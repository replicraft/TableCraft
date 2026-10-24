use super::*;
use crate::{evaluation::NoBindings, functions::text::byte_string::ByteString};
use proptest::prelude::*;
use std::cell::Cell;
use tablecraft_core::limits::ResourceBudget;
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
fn cstr_retains_string_bytes_without_weakening_scalar_conversion_or_publication() {
    for (sql, expected) in [
        ("Len(CStr(ChrW(55296)))", Value::Int32(1)),
        ("AscW(CStr(ChrW(55296)))", Value::Int16(-10240)),
        ("LenB(CStr(ChrB(65)))", Value::Int32(1)),
        ("AscB(CStr(CStr(ChrB(65))))", Value::Int16(65)),
        ("TypeName(CStr(ChrB(65)))", Value::Text("String".into())),
        ("VarType(CStr(ChrW(55296)))", Value::Int16(8)),
        (
            "CStr(ChrW(55357))&CStr(ChrW(56832))",
            Value::Text("😀".into()),
        ),
        ("CStr(ChrW(55357)&ChrW(56832))", Value::Text("😀".into())),
        ("Left(CStr(ChrW(55296)),0)", Value::Text(String::new())),
        ("Len(CStr(CVErr(11)))", Value::Int32(8)),
    ] {
        assert_eq!(evaluate(sql), Ok(expected), "{sql}");
    }

    for (sql, expected) in [
        ("CStr(ChrW(55296))", EvalError::UnrepresentableText),
        ("CStr(ChrB(65))", EvalError::UnrepresentableText),
        ("Len(CStr(ChrB(65)))", EvalError::UnrepresentableText),
        ("Len(CStr(NULL))", EvalError::InvalidNullConversion),
        ("CStr(1)", EvalError::NumberFormatRequired),
        ("CStr(1/0)", EvalError::DivisionByZero),
        ("CStr(ChrW(55296),1/0)", EvalError::UnsupportedExpression),
    ] {
        assert_eq!(evaluate(sql), Err(expected), "{sql}");
    }
}

#[test]
fn cstr_moves_an_existing_range_without_allocating_and_remains_cancellable() {
    let raw = ByteString::from_owned(vec![0, 65, 0, 66])
        .unwrap()
        .mid(2, Some(1))
        .unwrap();
    let pointer = raw.as_bytes().as_ptr();
    let mut budget = ResourceBudget::new(0, 0, 100);
    let converted = Evaluator::new(&NoBindings, &mut budget, Default::default())
        .apply_expression_cstr_operand(ExpressionByteOperand::Bytes(raw))
        .unwrap();
    let ByteOperand::Bytes(converted) = converted else {
        panic!("raw string required");
    };
    assert_eq!(converted.as_bytes(), &[65]);
    assert_eq!(converted.as_bytes().as_ptr(), pointer);

    let expression =
        parse_expression("LenB(CStr(CStr(LeftB('😀',1))))", Default::default()).unwrap();
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
    fn cstr_retains_every_supplied_string_byte(bytes in prop::collection::vec(any::<u8>(), 0..128)) {
        let expected = bytes.clone();
        let raw = ByteString::from_owned(bytes).unwrap();
        let mut budget = ResourceBudget::new(0, 0, 100);
        let converted = Evaluator::new(&NoBindings, &mut budget, Default::default())
            .apply_expression_cstr_operand(ExpressionByteOperand::Bytes(raw))
            .unwrap();
        let ByteOperand::Bytes(converted) = converted else {
            panic!("raw string required");
        };
        prop_assert_eq!(converted.as_bytes(), expected);
    }
}

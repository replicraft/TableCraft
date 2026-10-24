use crate::{
    EvalError,
    evaluation::{EvalLimits, Evaluator, NoBindings},
    functions::{
        conversions::text::NumberFormat,
        text::{
            TextFunction,
            byte_string::{ByteOperand, ByteString},
        },
    },
};
use proptest::prelude::*;
use std::cell::Cell;
use tablecraft_core::{
    limits::ResourceBudget,
    values::{ExpressionString, ExpressionValue, Value},
};
use tablecraft_sql::parse_expression;

fn evaluate(sql: &str) -> Result<Value, EvalError> {
    let expression = parse_expression(sql, Default::default()).unwrap();
    let mut budget = ResourceBudget::new(65536, 0, 100000);
    Evaluator::new(&NoBindings, &mut budget, Default::default())
        .with_number_format(NumberFormat::EnUs)
        .evaluate(&expression)
}

fn raw(units: &[u16]) -> ExpressionValue {
    ExpressionValue::StringBytes(ExpressionString::from_owned(
        units.iter().flat_map(|unit| unit.to_le_bytes()).collect(),
    ))
}

fn count(value: i32) -> ExpressionValue {
    ExpressionValue::Value(Value::Int32(value))
}

#[test]
fn string_repeats_the_first_unit_without_publishing_source_tails() {
    for (sql, expected) in [
        ("LenB(String(3,ChrW(55296)))", Value::Int32(6)),
        ("AscW(String(2,'😀'))", Value::Int16(-10179)),
        (
            "StrComp(String(2,ChrW(57343)),ChrW(57343)&ChrW(57343),0)",
            Value::Int16(0),
        ),
        ("String(2,ChrW(65)&ChrW(55296))", Value::Text("AA".into())),
        ("String(2,MidB('😀',2,2))", Value::Text("ØØ".into())),
        ("String(0,'😀')", Value::Text(String::new())),
        (
            "String(1,ChrW(55357))&ChrW(56832)",
            Value::Text("😀".into()),
        ),
        ("LenB(String('2.5',ChrW(55296)))", Value::Int32(4)),
        ("LenB(String(ChrW(50),ChrW(55296)))", Value::Int32(4)),
        ("String(NULL,ChrB(65))", Value::Null),
        ("String(ChrB(65),NULL)", Value::Null),
    ] {
        assert_eq!(evaluate(sql), Ok(expected), "{sql}");
    }
}

#[test]
fn string_validates_count_and_character_after_null_but_keeps_operational_errors_eager() {
    for (sql, expected) in [
        ("String(2,ChrW(55296))", EvalError::UnrepresentableText),
        ("String(0,ChrB(65))", EvalError::UnrepresentableText),
        ("String(0,LeftB('',0))", EvalError::Domain),
        ("String(-1,ChrB(65))", EvalError::Domain),
        (
            "String(ChrB(65),ChrW(55296))",
            EvalError::UnrepresentableText,
        ),
        ("String(0,CVErr(11))", EvalError::TypeMismatch),
        ("String(NULL,1/0)", EvalError::DivisionByZero),
        ("String(1/0,NULL)", EvalError::DivisionByZero),
        ("String(0,128)", EvalError::CharacterPolicyRequired),
    ] {
        assert_eq!(evaluate(sql), Err(expected), "{sql}");
    }
}

#[test]
fn retained_and_scalar_entrypoints_share_literal_and_numeric_policies() {
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, Default::default());
    let empty = evaluator
        .apply_retained_string(&count(0), &raw(&[0xd800]))
        .unwrap();
    assert_eq!(
        evaluator.publish_byte_operand(empty),
        Ok(Value::Text(String::new()))
    );
    let empty_character = evaluator
        .apply_retained_string(&count(2), &ExpressionValue::Empty)
        .unwrap();
    assert_eq!(
        evaluator.publish_byte_operand(empty_character),
        Ok(Value::Text("\0\0".into()))
    );
    assert_eq!(
        evaluator.apply_expression_repeat(TextFunction::String, &[count(1), raw(&[0xd800])]),
        Err(EvalError::UnrepresentableText),
    );
    let direct = evaluator
        .apply_byte_string(
            TextFunction::String,
            ByteOperand::Scalar(Value::Int32(2)),
            &[Value::Text("😀".into())],
        )
        .unwrap();
    assert!(matches!(direct, ByteOperand::Bytes(bytes) if bytes.as_bytes() == [61, 216, 61, 216]));
    let raw_count = ByteString::from_owned(vec![50, 0]).unwrap();
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let direct = Evaluator::new(&NoBindings, &mut budget, Default::default())
        .with_number_format(NumberFormat::EnUs)
        .apply_byte_string(
            TextFunction::String,
            ByteOperand::Bytes(raw_count),
            &[Value::Text("x".into())],
        )
        .unwrap();
    assert!(matches!(direct, ByteOperand::Scalar(Value::Text(text)) if text == "xx"));
}

#[test]
fn raw_string_output_and_every_observed_checkpoint_share_limits() {
    let character = raw(&[0xd800]);
    let total = Cell::new(0);
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    Evaluator::new(&NoBindings, &mut budget, Default::default())
        .with_cancellation(&|| {
            total.set(total.get() + 1);
            false
        })
        .apply_retained_string(&count(4), &character)
        .unwrap();
    for stop in 1..=total.get() {
        let calls = Cell::new(0);
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        assert!(matches!(
            Evaluator::new(&NoBindings, &mut budget, Default::default())
                .with_cancellation(&|| {
                    calls.set(calls.get() + 1);
                    calls.get() >= stop
                })
                .apply_retained_string(&count(4), &character),
            Err(EvalError::Cancelled)
        ));
    }
    for (bytes, steps) in [(0, 10000), (7, 10000), (65536, 0)] {
        let mut budget = ResourceBudget::new(bytes, 0, steps);
        assert!(matches!(
            Evaluator::new(&NoBindings, &mut budget, Default::default())
                .apply_retained_string(&count(4), &character),
            Err(EvalError::Limit(_))
        ));
    }
    let mut budget = ResourceBudget::new(0, 0, 10000);
    let empty = Evaluator::new(&NoBindings, &mut budget, Default::default())
        .apply_retained_string(&count(0), &character)
        .unwrap();
    assert!(matches!(empty, ByteOperand::Bytes(bytes) if bytes.is_empty()));

    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let oversized = ExpressionValue::Value(Value::Text("long".into()));
    assert!(matches!(
        Evaluator::new(
            &NoBindings,
            &mut budget,
            EvalLimits {
                max_text_bytes: 1,
                ..Default::default()
            }
        )
        .apply_retained_string(&ExpressionValue::Value(Value::Null), &oversized),
        Err(EvalError::TextLimit)
    ));
}

proptest! {
    #[test]
    fn selected_units_repeat_exactly_and_ignore_unselected_complete_tails(
        unit in any::<u16>(),
        tail in prop::collection::vec(any::<u16>(), 0..12),
        repetitions in 0i32..32,
    ) {
        let units = std::iter::once(unit).chain(tail).collect::<Vec<_>>();
        let mut budget = ResourceBudget::new(65536, 0, 100000);
        let result = Evaluator::new(&NoBindings, &mut budget, Default::default())
            .apply_retained_string(&count(repetitions), &raw(&units)).unwrap();
        let actual = match result {
            ByteOperand::Bytes(bytes) => bytes.as_bytes().to_vec(),
            ByteOperand::Scalar(Value::Text(text)) => text.encode_utf16().flat_map(u16::to_le_bytes).collect(),
            _ => panic!("String result required"),
        };
        let expected = unit.to_le_bytes().repeat(repetitions as usize);
        prop_assert_eq!(actual, expected);
    }
}

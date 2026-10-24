use crate::{
    EvalError,
    evaluation::{Evaluator, NoBindings},
    functions::text::{
        TextFunction,
        byte_string::{ByteOperand, ByteString, ExpressionByteOperand},
    },
};
use proptest::prelude::*;
use std::cell::Cell;
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
fn unit_reversal_retains_surrogates_until_an_observer_or_repaired_publication() {
    for (sql, expected) in [
        ("AscW(StrReverse('a'&ChrW(55296)))", Value::Int16(-10240)),
        ("LenB(StrReverse('🦀'))", Value::Int32(4)),
        ("AscW(StrReverse('🦀'))", Value::Int16(-8832)),
        (
            "StrReverse(StrReverse('a🦀中'))",
            Value::Text("a🦀中".into()),
        ),
        (
            "StrReverse(ChrW(56832)&ChrW(55357))",
            Value::Text("😀".into()),
        ),
        (
            "StrReverse(ChrW(55296))&ChrW(56320)",
            Value::Text("\u{10000}".into()),
        ),
    ] {
        assert_eq!(evaluate(sql), Ok(expected), "{sql}");
    }
}

#[test]
fn reversal_refuses_odd_units_and_strict_publication_without_replacement() {
    for sql in [
        "StrReverse('🦀')",
        "StrReverse(ChrW(55296))",
        "LenB(StrReverse(ChrB(1)))",
    ] {
        assert_eq!(evaluate(sql), Err(EvalError::UnrepresentableText), "{sql}");
    }
    assert_eq!(
        evaluate("StrReverse(NULL)"),
        Err(EvalError::InvalidNullConversion)
    );
    assert_eq!(evaluate("StrReverse(1/0)"), Err(EvalError::DivisionByZero));
}

#[test]
fn raw_unaligned_ranges_use_only_their_active_complete_units() {
    let bytes = ByteString::from_owned(vec![99, 65, 0, 0, 216, 99])
        .unwrap()
        .mid(2, Some(4))
        .unwrap();
    let mut budget = ResourceBudget::new(4, 0, 100);
    let result = Evaluator::new(&NoBindings, &mut budget, Default::default())
        .apply_expression_byte_string(
            TextFunction::StrReverse,
            ExpressionByteOperand::Bytes(bytes),
            &[],
        )
        .unwrap();
    let ByteOperand::Bytes(bytes) = result else {
        panic!("retained raw result required");
    };
    assert_eq!(bytes.as_bytes(), [0, 216, 65, 0]);
}

#[test]
fn expression_states_and_payload_limits_remain_owned_by_reversal() {
    for (value, expected) in [
        (ExpressionValue::Error(11), EvalError::TypeMismatch),
        (ExpressionValue::Missing, EvalError::NonScalarExpression),
    ] {
        let mut budget = ResourceBudget::new(100, 0, 100);
        let result = Evaluator::new(&NoBindings, &mut budget, Default::default())
            .apply_expression_byte_string(
                TextFunction::StrReverse,
                ExpressionByteOperand::Scalar(value),
                &[],
            );
        assert!(matches!(result, Err(error) if error == expected));
    }

    let mut budget = ResourceBudget::new(0, 0, 100);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, Default::default());
    let empty = evaluator.apply_expression_byte_string(
        TextFunction::StrReverse,
        ExpressionByteOperand::Scalar(ExpressionValue::Empty),
        &[],
    );
    assert_eq!(
        evaluator.publish_byte_operand(empty.unwrap()),
        Ok(Value::Text(String::new()))
    );

    let mut budget = ResourceBudget::new(3, 0, 100);
    let result = Evaluator::new(&NoBindings, &mut budget, Default::default()).apply_byte_string(
        TextFunction::StrReverse,
        ByteOperand::Bytes(ByteString::from_owned(vec![1, 0, 2, 0]).unwrap()),
        &[],
    );
    assert!(matches!(result, Err(EvalError::Limit(_))));
}

#[test]
fn every_observed_reversal_checkpoint_can_cancel_without_a_result() {
    let count = Cell::new(0);
    let stop = Cell::new(u32::MAX);
    let cancelled = || {
        count.set(count.get() + 1);
        count.get() == stop.get()
    };
    let run = || {
        let mut budget = ResourceBudget::new(100, 0, 1000);
        Evaluator::new(&NoBindings, &mut budget, Default::default())
            .with_cancellation(&cancelled)
            .apply_byte_string(
                TextFunction::StrReverse,
                ByteOperand::Bytes(ByteString::from_owned(vec![1, 0, 2, 0, 3, 0]).unwrap()),
                &[],
            )
    };
    run().unwrap();
    let total = count.get();
    for checkpoint in 1..=total {
        count.set(0);
        stop.set(checkpoint);
        assert!(matches!(run(), Err(EvalError::Cancelled)));
    }
}

proptest! {
    #![proptest_config(crate::test_support::config())]

    #[test]
    fn arbitrary_unit_buffers_reverse_without_decoding_and_form_an_involution(
        units in prop::collection::vec(any::<u16>(), 0..40),
    ) {
        let original = units
            .iter()
            .flat_map(|unit| unit.to_le_bytes())
            .collect::<Vec<_>>();
        let mut budget = ResourceBudget::new(10000, 0, 10000);
        let mut evaluator = Evaluator::new(&NoBindings, &mut budget, Default::default());
        let result = evaluator
            .apply_byte_string(
                TextFunction::StrReverse,
                ByteOperand::Bytes(ByteString::from_owned(original.clone()).unwrap()),
                &[],
            )
            .unwrap();
        let ByteOperand::Bytes(ref reversed) = result else {
            panic!("raw result required");
        };

        let (pairs, remainder) = reversed.as_bytes().as_chunks::<2>();
        prop_assert!(remainder.is_empty());
        for (position, pair) in pairs.iter().enumerate() {
            let actual = u16::from_le_bytes(*pair);
            prop_assert_eq!(actual, units[units.len() - position - 1]);
        }

        let result = evaluator
            .apply_byte_string(TextFunction::StrReverse, result, &[])
            .unwrap();
        let ByteOperand::Bytes(restored) = result else {
            panic!("raw result required");
        };
        prop_assert_eq!(restored.as_bytes(), original);
    }
}

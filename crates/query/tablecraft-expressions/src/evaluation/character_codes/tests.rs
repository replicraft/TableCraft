use super::*;
use crate::{
    evaluation::{EvalLimits, NoBindings},
    functions::{
        conversions::text::NumberFormat,
        text::{byte_string::ByteOperand, code_page::CharacterCodePage},
    },
};
use std::cell::Cell;
use tablecraft_core::limits::ResourceBudget;

#[test]
fn code_normalization_and_character_output_share_budget_and_explicit_page() {
    let arguments = [Value::Text("65.5".into())];
    let mut budget = ResourceBudget::new(5, 0, 1000);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_number_format(NumberFormat::EnUs)
            .apply_text_function(TextFunction::Chr, &arguments),
        Ok(Value::Text("B".into()))
    );
    let mut budget = ResourceBudget::new(1000, 0, 1000);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_number_format(NumberFormat::EnUs)
            .with_character_code_page(CharacterCodePage::from_windows_id(1252).unwrap())
            .apply_text_function(TextFunction::Chr, &[Value::Text("128".into())]),
        Ok(Value::Text("€".into()))
    );
    let mut budget = ResourceBudget::new(3, 0, 1000);
    let value = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .with_number_format(NumberFormat::EnUs)
        .apply_byte_string(
            TextFunction::ChrB,
            ByteOperand::Scalar(Value::Text("66".into())),
            &[],
        )
        .unwrap();
    let ByteOperand::Bytes(value) = value else {
        panic!("expected a raw string byte");
    };
    assert_eq!(value.as_bytes(), &[66]);
}

#[test]
fn every_code_conversion_and_output_checkpoint_can_cancel() {
    let arguments = [Value::Text("65.5".into())];
    let observed = Cell::new(0);
    let callback = || {
        observed.set(observed.get() + 1);
        false
    };
    let mut budget = ResourceBudget::new(1000, 0, 1000);
    Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .with_number_format(NumberFormat::EnUs)
        .with_cancellation(&callback)
        .apply_text_function(TextFunction::ChrW, &arguments)
        .unwrap();
    for stop in 1..=observed.get() {
        let current = Cell::new(0);
        let callback = || {
            current.set(current.get() + 1);
            current.get() == stop
        };
        let mut budget = ResourceBudget::new(1000, 0, 1000);
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_number_format(NumberFormat::EnUs)
                .with_cancellation(&callback)
                .apply_text_function(TextFunction::ChrW, &arguments),
            Err(EvalError::Cancelled)
        );
    }
}

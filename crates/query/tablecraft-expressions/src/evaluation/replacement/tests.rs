use super::*;
use crate::{
    evaluation::{EvalLimits, NoBindings},
    functions::{conversions::text::NumberFormat, text::TextFunction},
};
use std::cell::Cell;
use tablecraft_core::limits::ResourceBudget;

#[test]
fn normalization_and_replacement_output_share_bytes_without_copying_sources() {
    let arguments = [
        Value::Text("banana".into()),
        Value::Text("an".into()),
        Value::Text("X".into()),
        Value::Text("2,5".into()),
        Value::Text("2,5".into()),
    ];
    let mut budget = ResourceBudget::new(9, 0, 1000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .with_number_format(NumberFormat::DeDe);
    assert_eq!(
        evaluator.apply_text_function(TextFunction::Replace, &arguments),
        Ok(Value::Text("XXa".into()))
    );
    assert!(matches!(
        evaluator.apply_text_function(TextFunction::Replace, &arguments),
        Err(EvalError::Limit(_))
    ));
}

#[test]
fn count_and_mode_validation_keep_their_order_before_source_null_checks() {
    let mut budget = ResourceBudget::new(1000, 0, 1000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .with_number_format(NumberFormat::EnUs);
    for (start, count, mode, expected) in [
        (
            Value::Text("bad".into()),
            Value::Int16(-2),
            Value::Int16(3),
            EvalError::TypeMismatch,
        ),
        (
            Value::Text("1".into()),
            Value::Null,
            Value::Int16(3),
            EvalError::InvalidNullConversion,
        ),
        (
            Value::Text("0".into()),
            Value::Int16(-2),
            Value::Null,
            EvalError::InvalidNullConversion,
        ),
        (
            Value::Text("0".into()),
            Value::Int16(-2),
            Value::Int16(0),
            EvalError::Domain,
        ),
        (
            Value::Text("1".into()),
            Value::Int16(0),
            Value::Int16(0),
            EvalError::InvalidNullConversion,
        ),
    ] {
        let arguments = [
            Value::Null,
            Value::Text("a".into()),
            Value::Text("b".into()),
            start,
            count,
            mode,
        ];
        assert_eq!(
            evaluator.apply_text_function(TextFunction::Replace, &arguments),
            Err(expected)
        );
    }
}

#[test]
fn every_count_scan_match_and_output_checkpoint_can_cancel() {
    let arguments = [
        Value::Text("banana".into()),
        Value::Text("an".into()),
        Value::Text("X".into()),
        Value::Text("1.5".into()),
        Value::Text("2.5".into()),
    ];
    let observed = Cell::new(0);
    let callback = || {
        observed.set(observed.get() + 1);
        false
    };
    let mut budget = ResourceBudget::new(1000, 0, 1000);
    Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .with_number_format(NumberFormat::EnUs)
        .with_cancellation(&callback)
        .apply_text_function(TextFunction::Replace, &arguments)
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
                .apply_text_function(TextFunction::Replace, &arguments),
            Err(EvalError::Cancelled)
        );
    }
}

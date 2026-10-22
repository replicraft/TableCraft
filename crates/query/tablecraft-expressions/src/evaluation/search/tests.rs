use super::*;
use crate::{
    evaluation::{EvalLimits, NoBindings},
    functions::conversions::text::NumberFormat,
};
use std::cell::Cell;
use tablecraft_core::limits::ResourceBudget;

#[test]
fn mode_and_null_validation_do_not_depend_on_text_start_parse_success() {
    let mut budget = ResourceBudget::new(1000, 0, 1000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .with_number_format(NumberFormat::EnUs);
    assert_eq!(
        evaluator.apply_text_function(
            TextFunction::InStrRev,
            &[
                Value::Text("abc".into()),
                Value::Text("b".into()),
                Value::Text("bad".into()),
                Value::Int16(3)
            ]
        ),
        Err(EvalError::Domain)
    );
    assert_eq!(
        evaluator.apply_text_function(
            TextFunction::InStr,
            &[
                Value::Text("bad".into()),
                Value::Null,
                Value::Text("b".into()),
                Value::Int16(0)
            ]
        ),
        Err(EvalError::TypeMismatch)
    );
    assert_eq!(
        evaluator.apply_text_function(
            TextFunction::InStr,
            &[
                Value::Text("1".into()),
                Value::Null,
                Value::Text("b".into()),
                Value::Int16(0)
            ]
        ),
        Ok(Value::Null)
    );
}

#[test]
fn search_conversion_borrows_strings_and_cancels_at_each_checkpoint() {
    let arguments = [
        Value::Text("2.5".into()),
        Value::Text("ababab".into()),
        Value::Text("ab".into()),
        Value::Int16(0),
    ];
    let mut budget = ResourceBudget::new(3, 0, 1000);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_number_format(NumberFormat::EnUs)
            .apply_text_function(TextFunction::InStr, &arguments),
        Ok(Value::Int32(3))
    );
    let seen = Cell::new(0);
    let observed = || {
        seen.set(seen.get() + 1);
        false
    };
    let mut budget = ResourceBudget::new(1000, 0, 1000);
    Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .with_number_format(NumberFormat::EnUs)
        .with_cancellation(&observed)
        .apply_text_function(TextFunction::InStr, &arguments)
        .unwrap();
    for stop in 1..=seen.get() {
        let current = Cell::new(0);
        let cancelled = || {
            current.set(current.get() + 1);
            current.get() == stop
        };
        let mut budget = ResourceBudget::new(1000, 0, 1000);
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_number_format(NumberFormat::EnUs)
                .with_cancellation(&cancelled)
                .apply_text_function(TextFunction::InStr, &arguments),
            Err(EvalError::Cancelled)
        );
    }
}

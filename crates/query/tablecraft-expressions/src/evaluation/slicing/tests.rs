use super::*;
use crate::{
    evaluation::{EvalLimits, NoBindings},
    functions::conversions::text::NumberFormat,
};
use std::cell::Cell;
use tablecraft_core::limits::ResourceBudget;

#[test]
fn contextual_count_buffers_and_result_share_one_budget_without_copying_source() {
    let arguments = [Value::Text("abcdef".into()), Value::Text("2,5".into())];
    let mut budget = ResourceBudget::new(5, 0, 1000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .with_number_format(NumberFormat::DeDe);
    assert_eq!(
        evaluator.apply_text_function(TextFunction::Left, &arguments),
        Ok(Value::Text("ab".into()))
    );
    assert!(matches!(
        evaluator.apply_text_function(TextFunction::Left, &arguments),
        Err(EvalError::Limit(_))
    ));
    let mut budget = ResourceBudget::new(1000, 0, 1000);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .apply_text_function(TextFunction::Left, &arguments),
        Err(EvalError::NumberFormatRequired),
    );
}

#[test]
fn each_text_conversion_scan_and_output_checkpoint_can_cancel() {
    let arguments = [
        Value::Text("abcde".into()),
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
        .apply_text_function(TextFunction::Mid, &arguments)
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
                .apply_text_function(TextFunction::Mid, &arguments),
            Err(EvalError::Cancelled),
        );
    }
}

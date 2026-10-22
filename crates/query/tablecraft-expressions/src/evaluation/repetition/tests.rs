use super::*;
use crate::{
    evaluation::{EvalLimits, NoBindings},
    functions::conversions::text::NumberFormat,
};
use tablecraft_core::limits::ResourceBudget;

#[test]
fn text_count_conversion_never_reinterprets_the_string_character_argument() {
    let mut budget = ResourceBudget::new(1000, 0, 1000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .with_number_format(NumberFormat::DeDe);
    assert_eq!(
        evaluator.apply_text_function(
            TextFunction::String,
            &[Value::Text("2,5".into()), Value::Text("65".into())]
        ),
        Ok(Value::Text("66".into()))
    );
    assert_eq!(
        evaluator.apply_text_function(TextFunction::Space, &[Value::Text("2,5".into())]),
        Ok(Value::Text("  ".into()))
    );
    assert_eq!(
        evaluator.apply_text_function(
            TextFunction::String,
            &[Value::Text("bad".into()), Value::Null]
        ),
        Ok(Value::Null)
    );
}

#[test]
fn normalization_and_repeat_output_share_cumulative_bytes() {
    let mut budget = ResourceBudget::new(5, 0, 1000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .with_number_format(NumberFormat::DeDe);
    let arguments = [Value::Text("2,5".into()), Value::Text("abcdef".into())];
    assert_eq!(
        evaluator.apply_text_function(TextFunction::String, &arguments),
        Ok(Value::Text("aa".into()))
    );
    assert!(matches!(
        evaluator.apply_text_function(TextFunction::String, &arguments),
        Err(EvalError::Limit(_))
    ));
}

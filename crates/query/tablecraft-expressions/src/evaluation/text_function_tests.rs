use super::*;
use crate::{EvalError, functions::text::TextFunction};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_sql::{ParseLimits, parse_select, select::Projection};

#[test]
fn text_scan_output_and_input_limits_share_the_evaluator_budget() {
    let arguments = [Value::Text("abcdef".into()), Value::Int64(2)];
    let mut budget = ResourceBudget::new(2, 0, 100);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    assert_eq!(
        evaluator.apply_text_function(TextFunction::Left, &arguments),
        Ok(Value::Text("ab".into()))
    );
    assert!(
        matches!(evaluator.apply_text_function(TextFunction::Left,&arguments),Err(EvalError::Limit(error)) if error.resource == Resource::Bytes)
    );
    let mut budget = ResourceBudget::new(100, 0, 1);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    assert!(
        matches!(evaluator.apply_text_function(TextFunction::Len,&arguments[..1]),Err(EvalError::Limit(error)) if error.resource == Resource::Steps)
    );
    let mut budget = ResourceBudget::new(100, 0, 100);
    let mut evaluator = Evaluator::new(
        &NoBindings,
        &mut budget,
        EvalLimits {
            max_text_bytes: 3,
            ..EvalLimits::default()
        },
    );
    assert_eq!(
        evaluator.apply_text_function(TextFunction::Left, &arguments),
        Err(EvalError::TextLimit)
    );
    let mut evaluator = evaluator.with_cancellation(&|| true);
    assert_eq!(
        evaluator.apply_text_function(TextFunction::Len, &[Value::Text("x".into())]),
        Err(EvalError::Cancelled)
    );
}
#[test]
fn invalid_text_call_shapes_do_not_evaluate_operands_and_null_still_evaluates_positions() {
    let mut budget = ResourceBudget::new(100, 0, 100);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    for (sql, error) in [
        ("SELECT Len(1/0,1)", EvalError::UnsupportedExpression),
        ("SELECT Mid('abc',1,)", EvalError::UnsupportedExpression),
        ("SELECT VBA.Trim('x')", EvalError::UnsupportedExpression),
        ("SELECT Left(Null,1/0)", EvalError::DivisionByZero),
    ] {
        let parsed = parse_select(sql, ParseLimits::default()).unwrap();
        let Projection::Expression { expression, .. } = &parsed.query().projection[0] else {
            panic!()
        };
        assert_eq!(evaluator.evaluate(expression), Err(error));
    }
}

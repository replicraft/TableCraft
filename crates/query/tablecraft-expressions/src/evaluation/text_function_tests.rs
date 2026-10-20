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
        (
            "SELECT InStr(1,'a','a',0,1/0)",
            EvalError::UnsupportedExpression,
        ),
        (
            "SELECT InStrRev('a','a',1,0,1/0)",
            EvalError::UnsupportedExpression,
        ),
        (
            "SELECT Replace('a','a',1/0,1,1,0,1)",
            EvalError::UnsupportedExpression,
        ),
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

#[test]
fn strcomp_has_int16_results_and_keeps_scan_cancellation_and_optional_slot_rules() {
    let arguments = [
        Value::Text("abcd".into()),
        Value::Text("abcd".into()),
        Value::Int16(0),
    ];
    let mut budget = ResourceBudget::new(100, 0, 2);
    assert!(
        matches!(Evaluator::new(&NoBindings,&mut budget,EvalLimits::default()).apply_text_function(TextFunction::StrComp,&arguments),Err(EvalError::Limit(error)) if error.resource==Resource::Steps)
    );
    let mut budget = ResourceBudget::new(100, 0, 100);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_cancellation(&|| true)
            .apply_text_function(TextFunction::StrComp, &arguments),
        Err(EvalError::Cancelled)
    );
    let parsed = parse_select("SELECT StrComp('ABCD','abcd',0)", ParseLimits::default()).unwrap();
    let Projection::Expression { expression, .. } = &parsed.query().projection[0] else {
        panic!("expression")
    };
    let mut budget = ResourceBudget::new(100, 0, 100);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default()).evaluate(expression),
        Ok(Value::Int16(-1))
    );
}

#[test]
fn replacement_streaming_charges_payload_before_growth_and_keeps_cancel_checks() {
    let arguments = [
        Value::Text("aaaa".into()),
        Value::Text("a".into()),
        Value::Text("zz".into()),
    ];
    let mut budget = ResourceBudget::new(100, 0, 1000);
    let mut evaluator = Evaluator::new(
        &NoBindings,
        &mut budget,
        EvalLimits {
            max_text_bytes: 4,
            ..Default::default()
        },
    );
    assert_eq!(
        evaluator.apply_text_function(TextFunction::Replace, &arguments),
        Err(EvalError::TextLimit)
    );
    let mut budget = ResourceBudget::new(5, 0, 1000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    assert!(
        matches!(evaluator.apply_text_function(TextFunction::Replace,&arguments),Err(EvalError::Limit(error)) if error.resource==Resource::Bytes)
    );
    let cancelled = std::cell::Cell::new(0usize);
    let cancel = || {
        cancelled.set(cancelled.get() + 1);
        cancelled.get() > 12
    };
    let mut budget = ResourceBudget::new(100, 0, 1000);
    let mut evaluator =
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default()).with_cancellation(&cancel);
    assert_eq!(
        evaluator.apply_text_function(TextFunction::Replace, &arguments),
        Err(EvalError::Cancelled)
    );
}

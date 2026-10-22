use super::*;
use crate::{
    evaluation::{Bindings, EvalLimits, NoBindings},
    functions::{
        conversions::text::NumberFormat, dates::clock::ClockReading, math::random::RndState,
    },
};
use std::cell::Cell;
use tablecraft_core::{limits::ResourceBudget, values::ExtendedDateTime};
use tablecraft_sql::select::FieldReference;

fn evaluate(text: &str) -> Result<ExpressionValue, EvalError> {
    let mut budget = ResourceBudget::new(16 * 1024 * 1024, 0, 100_000);
    Evaluator::new(&NoBindings, &mut budget, Default::default())
        .apply_eval_expression(&Value::Text(text.into()))
}

#[test]
fn closed_expressions_use_access_operators_and_retain_expression_states() {
    for (text, expected) in [
        ("15+3", ExpressionValue::Value(Value::Int64(18))),
        ("Chr$(65)", ExpressionValue::Value(Value::Text("A".into()))),
        ("2 In (1,2)", ExpressionValue::Value(Value::Boolean(true))),
        (
            "2 Between 1 And 3",
            ExpressionValue::Value(Value::Boolean(true)),
        ),
        ("'a;b'", ExpressionValue::Value(Value::Text("a;b".into()))),
        ("'中🦀'", ExpressionValue::Value(Value::Text("中🦀".into()))),
        ("Eval('15+3')", ExpressionValue::Value(Value::Int64(18))),
        ("Eval('CVErr(2001)')", ExpressionValue::Error(2001)),
        (
            "IsError(Eval('CVErr(2001)'))",
            ExpressionValue::Value(Value::Boolean(true)),
        ),
        (
            "LBound(Array(1,2))",
            ExpressionValue::Value(Value::Int32(0)),
        ),
        ("Null", ExpressionValue::Value(Value::Null)),
        (
            "1-- trailing comment",
            ExpressionValue::Value(Value::Int64(1)),
        ),
    ] {
        assert_eq!(evaluate(text), Ok(expected), "{text}");
    }
    assert_eq!(evaluate("1/0"), Err(EvalError::DivisionByZero));
    for text in [
        "",
        "15+",
        "1;",
        "1; SELECT 2",
        "SELECT 1",
        "1) + (2",
        "UPDATE Items SET N=1",
    ] {
        assert_eq!(evaluate(text), Err(EvalError::DynamicSyntax), "{text}");
    }
}

struct ForbiddenBindings;
impl Bindings for ForbiddenBindings {
    fn resolve(&self, _: &FieldReference) -> Result<&Value, EvalError> {
        panic!("closed Eval must not request host bindings")
    }
}

#[test]
fn preflight_refuses_dynamic_names_and_unsupported_calls_before_effects() {
    for text in ["[Secret]", "IIf(False,[Secret],1)"] {
        let mut budget = ResourceBudget::new(1_000_000, 0, 10000);
        assert_eq!(
            Evaluator::new(&ForbiddenBindings, &mut budget, Default::default())
                .apply_eval_expression(&Value::Text(text.into())),
            Err(EvalError::UnboundField)
        );
    }
    for text in [
        "Forms!Orders!Total",
        "(SELECT 1)",
        "Exists (SELECT 1)",
        "UserCode()",
        "Shell('x')",
        "DSum('N','Items')",
        "VBA.Chr(65)",
    ] {
        assert!(
            matches!(
                evaluate(text),
                Err(EvalError::UnsupportedExpression | EvalError::DynamicSyntax)
            ),
            "{text}"
        );
    }
    let state = RndState::default();
    let mut budget = ResourceBudget::new(1_000_000, 0, 10000);
    let result = Evaluator::new(&NoBindings, &mut budget, Default::default())
        .with_random(&state)
        .apply_eval_expression(&Value::Text("Rnd()+UserCode()".into()));
    assert_eq!(result, Err(EvalError::UnsupportedExpression));
    assert_eq!(state.apply(None), RndState::default().apply(None));
}

#[test]
fn nested_context_is_shared_and_repeatable_mode_refuses_hidden_effects() {
    let state = RndState::default();
    let reference = RndState::default();
    let clock =
        ClockReading::new(ExtendedDateTime::new(2026, 10, 21, 13, 14, 15, 0).unwrap()).unwrap();
    let mut budget = ResourceBudget::new(1_000_000, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, Default::default())
        .with_clock(&clock)
        .with_random(&state)
        .with_number_format(NumberFormat::Invariant);
    assert_eq!(
        evaluator.apply_eval_expression(&Value::Text("Eval('Rnd()')".into())),
        reference.apply(None).map(ExpressionValue::Value)
    );
    assert_eq!(
        evaluator.apply_eval_expression(&Value::Text("Rnd(0)".into())),
        reference
            .apply(Some(&Value::Int32(0)))
            .map(ExpressionValue::Value)
    );
    assert_eq!(
        evaluator.apply_eval_expression(&Value::Text("Month(Date())".into())),
        Ok(ExpressionValue::Value(Value::Int16(10)))
    );
    assert_eq!(
        evaluator.apply_eval_expression(&Value::Int32(15)),
        Ok(ExpressionValue::Value(Value::Int64(15)))
    );
    let state = RndState::default();
    let mut budget = ResourceBudget::new(1_000_000, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, Default::default())
        .with_clock(&clock)
        .with_random(&state)
        .with_repeatable_dynamic_expressions();
    for text in ["Rnd()", "Eval('Rnd()')", "Date()", "Eval('Timer()')"] {
        assert_eq!(
            evaluator.apply_eval_expression(&Value::Text(text.into())),
            Err(EvalError::DynamicNotRepeatable)
        );
    }
    assert_eq!(state.apply(None), RndState::default().apply(None));
}

#[test]
fn dynamic_parsing_reuses_depth_cancellation_and_cumulative_allowances() {
    let value = Value::Text("Eval('15+3')".into());
    let observed = Cell::new(0);
    let count = || {
        observed.set(observed.get() + 1);
        false
    };
    let mut budget = ResourceBudget::new(1_000_000, 0, 10000);
    Evaluator::new(&NoBindings, &mut budget, Default::default())
        .with_cancellation(&count)
        .apply_eval_expression(&value)
        .unwrap();
    for stop in 1..=observed.get() {
        let checks = Cell::new(0);
        let cancelled = || {
            checks.set(checks.get() + 1);
            checks.get() >= stop
        };
        let mut budget = ResourceBudget::new(1_000_000, 0, 10000);
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, Default::default())
                .with_cancellation(&cancelled)
                .apply_eval_expression(&value),
            Err(EvalError::Cancelled)
        );
    }
    for (bytes, steps) in [(0, 10000), (1_000_000, 1)] {
        let mut budget = ResourceBudget::new(bytes, 0, steps);
        assert!(matches!(
            Evaluator::new(&NoBindings, &mut budget, Default::default())
                .apply_eval_expression(&value),
            Err(EvalError::Limit(_))
        ));
    }
    let mut budget = ResourceBudget::new(10_000, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, Default::default());
    evaluator.apply_eval_expression(&value).unwrap();
    let mut exhausted = false;
    for _ in 0..100 {
        match evaluator.apply_eval_expression(&value) {
            Ok(_) => {}
            Err(EvalError::Limit(_)) => {
                exhausted = true;
                break;
            }
            result => panic!("unexpected result: {result:?}"),
        }
    }
    assert!(exhausted);
    assert!(matches!(
        evaluator.apply_eval_expression(&value),
        Err(EvalError::Limit(_))
    ));
    let mut budget = ResourceBudget::new(1_000_000, 0, 10000);
    assert_eq!(
        Evaluator::new(
            &NoBindings,
            &mut budget,
            EvalLimits {
                max_text_bytes: 2,
                ..Default::default()
            }
        )
        .apply_eval_expression(&value),
        Err(EvalError::TextLimit)
    );
    let mut text = "1".to_owned();
    for _ in 0..5 {
        text = format!("Eval('{}')", text.replace("'", "''"));
    }
    let mut budget = ResourceBudget::new(1_000_000, 0, 10000);
    assert!(matches!(
        Evaluator::new(
            &NoBindings,
            &mut budget,
            EvalLimits {
                max_depth: 4,
                ..Default::default()
            }
        )
        .apply_eval_expression(&Value::Text(text)),
        Err(EvalError::DepthLimit | EvalError::DynamicLimit)
    ));
    let mut budget = ResourceBudget::new(1_000_000, 0, 10000);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, Default::default())
            .apply_eval_expression(&Value::Int32(15)),
        Err(EvalError::NumberFormatRequired)
    );
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, Default::default())
            .apply_eval_expression(&Value::Null),
        Err(EvalError::InvalidNullConversion)
    );
}

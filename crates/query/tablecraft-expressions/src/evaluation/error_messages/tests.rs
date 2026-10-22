use super::*;
use crate::{
    evaluation::{EvalLimits, NoBindings},
    functions::{conversions::text::NumberFormat, error_messages::ErrorMessages},
};
use std::{
    cell::Cell,
    sync::atomic::{AtomicUsize, Ordering},
};
use tablecraft_core::{limits::ResourceBudget, values::Double};
use tablecraft_sql::{parse_select, select::Projection};

struct Messages {
    calls: AtomicUsize,
    message: Result<&'static str, EvalError>,
}

impl ErrorMessages for Messages {
    fn message(&self, _: u16) -> Result<&str, EvalError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        self.message
    }
}

fn evaluate(sql: &str, messages: Option<&dyn ErrorMessages>) -> Result<Value, EvalError> {
    let parsed = parse_select(sql, Default::default()).unwrap();
    let Projection::Expression { expression, .. } = &parsed.query().projection[0] else {
        panic!("expression required");
    };
    let mut budget = ResourceBudget::new(100_000, 0, 100_000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, Default::default());
    if let Some(messages) = messages {
        evaluator = evaluator.with_error_messages(messages);
    }
    evaluator.evaluate(expression)
}

#[test]
fn explicit_number_is_text_not_error_state_and_zero_needs_no_catalog() {
    let messages = Messages {
        calls: AtomicUsize::new(0),
        message: Ok("é中🦀"),
    };
    for sql in [
        "SELECT Error(11)",
        "SELECT Error$(11)",
        "SELECT Eval('Error(11)')",
    ] {
        assert_eq!(
            evaluate(sql, Some(&messages)),
            Ok(Value::Text("é中🦀".into()))
        );
    }
    assert_eq!(messages.calls.load(Ordering::Relaxed), 3);
    assert_eq!(
        evaluate("SELECT Error(0)", None),
        Ok(Value::Text(String::new()))
    );
    assert_eq!(
        evaluate("SELECT Error(11)", None),
        Err(EvalError::ErrorCatalogRequired)
    );
    for sql in ["SELECT Error()", "SELECT Error(1,1/0)"] {
        assert_eq!(
            evaluate(sql, Some(&messages)),
            Err(EvalError::UnsupportedExpression)
        );
    }
    assert_eq!(messages.calls.load(Ordering::Relaxed), 3);
    assert_eq!(
        evaluate("SELECT IIf(True,Error(11),1/0)", Some(&messages)),
        Err(EvalError::DivisionByZero)
    );
    assert_eq!(
        evaluate("SELECT Error(CVErr(11))", Some(&messages)),
        Err(EvalError::NonScalarExpression)
    );
    assert_eq!(
        evaluate("SELECT Error(NULL)", Some(&messages)),
        Err(EvalError::InvalidNullConversion)
    );
    assert_eq!(
        evaluate("SELECT Error(1/0)", Some(&messages)),
        Err(EvalError::DivisionByZero)
    );
    let failed = Messages {
        calls: AtomicUsize::new(0),
        message: Err(EvalError::ErrorCatalogFailed),
    };
    assert_eq!(
        evaluate("SELECT Error(42)", Some(&failed)),
        Err(EvalError::ErrorCatalogFailed)
    );
    assert_eq!(
        evaluate("SELECT Error(0)", Some(&failed)),
        Ok(Value::Text(String::new()))
    );
}

#[test]
fn conversion_uses_checked_long_then_standard_range_with_explicit_text_policy() {
    let messages = Messages {
        calls: AtomicUsize::new(0),
        message: Ok("known"),
    };
    for (input, expected) in [
        (
            Value::Double(Double::from_float(-0.5)),
            Ok(Value::Text(String::new())),
        ),
        (Value::Int32(65535), Ok(Value::Text("known".into()))),
        (Value::Int32(65536), Err(EvalError::Domain)),
        (Value::Int32(-1), Err(EvalError::Domain)),
        (Value::Int64(i64::MAX), Err(EvalError::Overflow)),
        (
            Value::Text("11".into()),
            Err(EvalError::NumberFormatRequired),
        ),
    ] {
        let mut budget = ResourceBudget::new(100_000, 0, 100_000);
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, Default::default())
                .with_error_messages(&messages)
                .apply_error_message(&input),
            expected
        );
    }
    let mut budget = ResourceBudget::new(100_000, 0, 100_000);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, Default::default())
            .with_number_format(NumberFormat::Invariant)
            .with_error_messages(&messages)
            .apply_error_message(&Value::Text("11".into())),
        Ok(Value::Text("known".into()))
    );
}

#[test]
fn output_is_charged_and_every_lookup_checkpoint_can_cancel() {
    let messages = Messages {
        calls: AtomicUsize::new(0),
        message: Ok("é中🦀"),
    };
    for (bytes, steps) in [(8, 100), (9, 0), (9, 1)] {
        let mut budget = ResourceBudget::new(bytes, 0, steps);
        assert!(matches!(
            Evaluator::new(&NoBindings, &mut budget, Default::default())
                .with_error_messages(&messages)
                .apply_error_message(&Value::Int16(11)),
            Err(EvalError::Limit(_))
        ));
    }
    let checks = Cell::new(0);
    let mut budget = ResourceBudget::new(9, 0, 100);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, Default::default())
            .with_error_messages(&messages)
            .with_cancellation(&|| {
                checks.set(checks.get() + 1);
                false
            })
            .apply_error_message(&Value::Int16(11)),
        Ok(Value::Text("é中🦀".into()))
    );
    for stop in 1..=checks.get() {
        let observed = Cell::new(0);
        let mut budget = ResourceBudget::new(9, 0, 100);
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, Default::default())
                .with_error_messages(&messages)
                .with_cancellation(&|| {
                    observed.set(observed.get() + 1);
                    observed.get() >= stop
                })
                .apply_error_message(&Value::Int16(11)),
            Err(EvalError::Cancelled)
        );
    }
    let mut budget = ResourceBudget::new(100, 0, 100);
    assert_eq!(
        Evaluator::new(
            &NoBindings,
            &mut budget,
            EvalLimits {
                max_text_bytes: 8,
                ..Default::default()
            }
        )
        .with_error_messages(&messages)
        .apply_error_message(&Value::Int16(11)),
        Err(EvalError::TextLimit)
    );
}

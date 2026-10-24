use super::*;
use crate::{
    evaluation::{EvalLimits, NoBindings},
    functions::{conversions::text::NumberFormat, math::random::RndState},
};
use std::cell::Cell;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_sql::{parse_select, select::Projection};

fn number(value: i32) -> ExpressionValue {
    ExpressionValue::Value(Value::Int32(value))
}

#[test]
fn hyperlink_states_keep_selector_validation_before_null_and_string_sources() {
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    for (arguments, expected) in [
        (
            [ExpressionValue::Error(11), number(6)],
            Err(EvalError::Domain),
        ),
        (
            [
                ExpressionValue::Value(Value::Null),
                ExpressionValue::Error(11),
            ],
            Err(EvalError::TypeMismatch),
        ),
        (
            [ExpressionValue::Missing, number(0)],
            Err(EvalError::NonScalarExpression),
        ),
        (
            [ExpressionValue::Empty, ExpressionValue::Empty],
            Ok(Value::Text(String::new())),
        ),
        ([number(1), number(0)], Err(EvalError::TypeMismatch)),
    ] {
        assert_eq!(
            evaluator.apply_expression_hyperlink_part(&arguments),
            expected
        );
    }
}

#[test]
fn hyperlink_text_selector_borrows_source_and_obeys_shared_bytes_and_cancellation() {
    let arguments = [
        ExpressionValue::Value(Value::Text("show#u#s#tip".into())),
        ExpressionValue::Value(Value::Text("2.5".into())),
    ];
    let mut budget = ResourceBudget::new(4, 0, 1000);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_number_format(NumberFormat::EnUs)
            .apply_expression_hyperlink_part(&arguments),
        Ok(Value::Text("u".into()))
    );
    let calls = Cell::new(0);
    let invoke = |stop| {
        calls.set(0);
        let cancelled = || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        };
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_number_format(NumberFormat::EnUs)
            .with_cancellation(&cancelled)
            .apply_expression_hyperlink_part(&arguments)
    };
    assert_eq!(invoke(usize::MAX), Ok(Value::Text("u".into())));
    let total = calls.get();
    for stop in 1..=total {
        assert_eq!(invoke(stop), Err(EvalError::Cancelled));
    }
}

#[test]
fn partition_and_hyperlink_states_keep_late_volatile_arguments_once_only() {
    for sql in [
        "SELECT Partition(CVErr(11),0,9,Rnd())",
        "SELECT HyperlinkPart(CVErr(11),Rnd())",
    ] {
        let parsed = parse_select(sql, Default::default()).unwrap();
        let Projection::Expression { expression, .. } = &parsed.query().projection[0] else {
            panic!("expression required");
        };
        let state = RndState::default();
        let expected = RndState::default();
        expected.apply(None).unwrap();
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_random(&state)
                .evaluate(expression),
            Err(EvalError::TypeMismatch)
        );
        assert_eq!(state.apply(None), expected.apply(None));
    }
}

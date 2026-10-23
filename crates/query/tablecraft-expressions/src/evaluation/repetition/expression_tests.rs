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
fn repetition_states_keep_null_count_domain_and_character_order() {
    let null = ExpressionValue::Value(Value::Null);
    let error = ExpressionValue::Error(11);
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());

    for (function, arguments, expected) in [
        (
            TextFunction::Space,
            vec![ExpressionValue::Empty],
            Ok(Value::Text(String::new())),
        ),
        (
            TextFunction::String,
            vec![number(2), ExpressionValue::Empty],
            Ok(Value::Text("\0\0".into())),
        ),
        (
            TextFunction::String,
            vec![null.clone(), error.clone()],
            Ok(Value::Null),
        ),
        (
            TextFunction::String,
            vec![error.clone(), null.clone()],
            Ok(Value::Null),
        ),
        (
            TextFunction::String,
            vec![number(-1), null],
            Ok(Value::Null),
        ),
        (
            TextFunction::Space,
            vec![error.clone()],
            Err(EvalError::TypeMismatch),
        ),
        (
            TextFunction::String,
            vec![number(-1), error.clone()],
            Err(EvalError::Domain),
        ),
        (
            TextFunction::String,
            vec![number(0), error],
            Err(EvalError::TypeMismatch),
        ),
        (
            TextFunction::String,
            vec![
                ExpressionValue::Empty,
                ExpressionValue::Value(Value::Text(String::new())),
            ],
            Err(EvalError::Domain),
        ),
        (
            TextFunction::Space,
            vec![ExpressionValue::Missing],
            Err(EvalError::NonScalarExpression),
        ),
    ] {
        assert_eq!(
            evaluator.apply_expression_repeat(function, &arguments),
            expected
        );
    }

    assert_eq!(
        evaluator.apply_expression_repeat(TextFunction::String, &[]),
        Err(EvalError::UnsupportedExpression)
    );
}

#[test]
fn repetition_states_share_borrowed_character_conversion_and_output_budgets() {
    let arguments = [
        ExpressionValue::Value(Value::Text("2,5".into())),
        ExpressionValue::Value(Value::Text("abcdef".into())),
    ];
    let mut budget = ResourceBudget::new(5, 0, 1000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .with_number_format(NumberFormat::DeDe);
    assert_eq!(
        evaluator.apply_expression_repeat(TextFunction::String, &arguments),
        Ok(Value::Text("aa".into()))
    );
    assert!(matches!(
        evaluator.apply_expression_repeat(TextFunction::String, &arguments),
        Err(EvalError::Limit(_))
    ));

    let calls = Cell::new(0);
    let invoke = |stop| {
        calls.set(0);
        let cancelled = || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        };
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_number_format(NumberFormat::DeDe)
            .with_cancellation(&cancelled)
            .apply_expression_repeat(TextFunction::String, &arguments)
    };
    assert_eq!(invoke(usize::MAX), Ok(Value::Text("aa".into())));
    let total = calls.get();
    for stop in 1..=total {
        assert_eq!(invoke(stop), Err(EvalError::Cancelled));
    }

    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let limits = EvalLimits {
        max_text_bytes: 3,
        ..EvalLimits::default()
    };
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, limits).apply_expression_repeat(
            TextFunction::String,
            &[ExpressionValue::Value(Value::Null), arguments[1].clone()],
        ),
        Err(EvalError::TextLimit)
    );
}

#[test]
fn repetition_sql_keeps_late_operands_eager_and_volatile_operands_once_only() {
    for (sql, expected) in [
        ("SELECT String(NULL,CVErr(11))", Ok(Value::Null)),
        ("SELECT String(CVErr(11),NULL)", Ok(Value::Null)),
        ("SELECT String(-1,CVErr(11))", Err(EvalError::Domain)),
        ("SELECT String(NULL,1/0)", Err(EvalError::DivisionByZero)),
        (
            "SELECT String(CVErr(11),CStr(NULL))",
            Err(EvalError::InvalidNullConversion),
        ),
    ] {
        let parsed = parse_select(sql, Default::default()).unwrap();
        let Projection::Expression { expression, .. } = &parsed.query().projection[0] else {
            panic!("expression required");
        };
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, EvalLimits::default()).evaluate(expression),
            expected
        );
    }

    let parsed = parse_select("SELECT String(CVErr(11),Rnd())", Default::default()).unwrap();
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

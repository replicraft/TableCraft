use super::*;
use crate::{
    evaluation::{EvalLimits, NoBindings},
    functions::{math::random::RndState, text::casing::CaseLocale},
};
use std::cell::Cell;
use tablecraft_core::{limits::ResourceBudget, values::Double};
use tablecraft_sql::{parse_select, select::Projection};

const FUNCTIONS: [TextFunction; 10] = [
    TextFunction::Len,
    TextFunction::LTrim,
    TextFunction::RTrim,
    TextFunction::Trim,
    TextFunction::LCase,
    TextFunction::UCase,
    TextFunction::Asc,
    TextFunction::AscW,
    TextFunction::StrReverse,
    TextFunction::Val,
];

#[test]
fn unary_string_sources_admit_empty_without_formatting_error_data() {
    let locale: CaseLocale = "und".parse().unwrap();
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let mut evaluator =
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default()).with_case_locale(&locale);

    for function in FUNCTIONS {
        assert_eq!(
            evaluator.apply_expression_string_function(function, &ExpressionValue::Error(11)),
            Err(EvalError::TypeMismatch),
            "{function:?}"
        );
        assert_eq!(
            evaluator.apply_expression_string_function(function, &ExpressionValue::Missing),
            Err(EvalError::NonScalarExpression),
            "{function:?}"
        );
        let expected = match function {
            TextFunction::Len => Ok(Value::Int32(0)),
            TextFunction::Asc | TextFunction::AscW => Err(EvalError::Domain),
            TextFunction::Val => Ok(Value::Double(Double::from_float(0.0))),
            _ => Ok(Value::Text(String::new())),
        };
        assert_eq!(
            evaluator.apply_expression_string_function(function, &ExpressionValue::Empty),
            expected,
            "{function:?}"
        );
        let expected = match function {
            TextFunction::Asc
            | TextFunction::AscW
            | TextFunction::StrReverse
            | TextFunction::Val => Err(EvalError::InvalidNullConversion),
            _ => Ok(Value::Null),
        };
        assert_eq!(
            evaluator
                .apply_expression_string_function(function, &ExpressionValue::Value(Value::Null)),
            expected,
            "{function:?}"
        );
    }
    assert_eq!(
        evaluator.apply_expression_string_function(TextFunction::CStr, &ExpressionValue::Error(11)),
        Err(EvalError::UnsupportedExpression)
    );
}

#[test]
fn unary_string_states_preserve_borrowing_policies_limits_and_cancellation() {
    let mut budget = ResourceBudget::new(3, 0, 1000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    let source = ExpressionValue::Value(Value::Text("   abc  ".into()));
    assert_eq!(
        evaluator.apply_expression_string_function(TextFunction::Trim, &source),
        Ok(Value::Text("abc".into()))
    );
    assert!(matches!(
        evaluator.apply_expression_string_function(TextFunction::Trim, &source),
        Err(EvalError::Limit(_))
    ));

    let locale: CaseLocale = "und".parse().unwrap();
    for function in FUNCTIONS {
        let calls = Cell::new(0);
        let invoke = |stop| {
            calls.set(0);
            let cancelled = || {
                calls.set(calls.get() + 1);
                calls.get() == stop
            };
            let mut budget = ResourceBudget::new(65536, 0, 10000);
            Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_case_locale(&locale)
                .with_cancellation(&cancelled)
                .apply_expression_string_function(
                    function,
                    &ExpressionValue::Value(Value::Text("12.5 abc".into())),
                )
        };
        assert!(invoke(usize::MAX).is_ok(), "{function:?}");
        let total = calls.get();
        for stop in 1..=total {
            assert_eq!(invoke(stop), Err(EvalError::Cancelled), "{function:?}");
        }
    }

    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    assert_eq!(
        evaluator.apply_expression_string_function(TextFunction::LCase, &ExpressionValue::Empty),
        Err(EvalError::CaseLocaleRequired)
    );
    assert_eq!(
        evaluator.apply_expression_string_function(
            TextFunction::Len,
            &ExpressionValue::Value(Value::Int32(1))
        ),
        Err(EvalError::NumberFormatRequired)
    );
    let limits = EvalLimits {
        max_text_bytes: 3,
        ..EvalLimits::default()
    };
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, limits)
            .apply_expression_string_function(TextFunction::Trim, &source),
        Err(EvalError::TextLimit)
    );
}

#[test]
fn unary_string_sql_preserves_operational_failures_and_once_only_selected_states() {
    let parsed = parse_select("SELECT Len(IIf(True,CVErr(11),Rnd()))", Default::default()).unwrap();
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

    let parsed = parse_select("SELECT Trim(IIf(True,CVErr(11),1/0))", Default::default()).unwrap();
    let Projection::Expression { expression, .. } = &parsed.query().projection[0] else {
        panic!("expression required");
    };
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default()).evaluate(expression),
        Err(EvalError::DivisionByZero)
    );
}

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
fn numeric_formatter_states_keep_null_policy_options_and_source_order() {
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .with_number_format(NumberFormat::EnUs);
    for (function, zero, integer_zero) in [
        (TextFunction::FormatNumber, "0.00", "0"),
        (TextFunction::FormatPercent, "0.00%", "0%"),
        (TextFunction::FormatCurrency, "$0.00", "$0"),
    ] {
        assert_eq!(
            evaluator.apply_expression_numeric_format(function, &[ExpressionValue::Empty]),
            Ok(Value::Text(zero.into()))
        );
        assert_eq!(
            evaluator.apply_expression_numeric_format(
                function,
                &std::array::from_fn::<_, 5, _>(|_| ExpressionValue::Empty)
            ),
            Ok(Value::Text(integer_zero.into()))
        );
        for (arguments, expected) in [
            (
                vec![
                    ExpressionValue::Value(Value::Null),
                    ExpressionValue::Error(11),
                ],
                EvalError::InvalidNullConversion,
            ),
            (
                vec![
                    ExpressionValue::Error(11),
                    number(256),
                    ExpressionValue::Error(12),
                ],
                EvalError::Domain,
            ),
            (
                vec![number(1), number(256), ExpressionValue::Missing],
                EvalError::Domain,
            ),
            (
                vec![number(1), number(2), number(3), ExpressionValue::Error(11)],
                EvalError::Domain,
            ),
            (
                vec![number(1), number(2), number(0), ExpressionValue::Error(11)],
                EvalError::TypeMismatch,
            ),
            (
                vec![
                    ExpressionValue::Error(11),
                    number(2),
                    number(0),
                    number(0),
                    number(0),
                ],
                EvalError::TypeMismatch,
            ),
            (
                vec![ExpressionValue::Missing],
                EvalError::NonScalarExpression,
            ),
        ] {
            assert_eq!(
                evaluator.apply_expression_numeric_format(function, &arguments),
                Err(expected)
            );
        }
    }

    let mut budget = ResourceBudget::new(65536, 0, 10000);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .apply_expression_numeric_format(
                TextFunction::FormatNumber,
                &[ExpressionValue::Error(11)]
            ),
        Err(EvalError::NumberFormatRequired)
    );
}

#[test]
fn numeric_formatter_states_borrow_sources_and_share_output_and_cancellation_limits() {
    let arguments = [
        ExpressionValue::Value(Value::Text("0000000001".into())),
        ExpressionValue::Value(Value::Text("2".into())),
    ];
    let mut budget = ResourceBudget::new(15, 0, 1000);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_number_format(NumberFormat::EnUs)
            .apply_expression_numeric_format(TextFunction::FormatNumber, &arguments),
        Ok(Value::Text("1.00".into()))
    );

    for function in [
        TextFunction::FormatNumber,
        TextFunction::FormatPercent,
        TextFunction::FormatCurrency,
    ] {
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
                .apply_expression_numeric_format(function, &arguments)
        };
        assert!(invoke(usize::MAX).is_ok());
        let total = calls.get();
        for stop in 1..=total {
            assert_eq!(invoke(stop), Err(EvalError::Cancelled));
        }
    }
}

#[test]
fn formatter_states_keep_late_operands_eager_and_once_only() {
    let parsed = parse_select("SELECT FormatNumber(CVErr(11),Rnd())", Default::default()).unwrap();
    let Projection::Expression { expression, .. } = &parsed.query().projection[0] else {
        panic!("expression required");
    };
    let state = RndState::default();
    let expected = RndState::default();
    expected.apply(None).unwrap();
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_number_format(NumberFormat::EnUs)
            .with_random(&state)
            .evaluate(expression),
        Err(EvalError::TypeMismatch)
    );
    assert_eq!(state.apply(None), expected.apply(None));
}

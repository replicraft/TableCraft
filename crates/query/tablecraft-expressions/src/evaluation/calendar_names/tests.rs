use super::*;
use crate::{
    evaluation::{EvalLimits, NoBindings},
    functions::{
        conversions::text::NumberFormat,
        dates::{FirstDayOfWeek, parsing::DateTextFormat},
        math::random::RndState,
    },
};
use std::cell::Cell;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_sql::{parse_select, select::Projection};

fn number(value: i32) -> ExpressionValue {
    ExpressionValue::Value(Value::Int32(value))
}

#[test]
fn date_and_name_states_evaluate_late_volatile_controls_once_before_admission() {
    for sql in [
        "SELECT FormatDateTime(CVErr(11),Rnd())",
        "SELECT MonthName(CVErr(11),Rnd())",
        "SELECT WeekdayName(CVErr(11),False,Rnd())",
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
                .with_date_format(DateTextFormat::Iso)
                .with_random(&state)
                .evaluate(expression),
            Err(EvalError::TypeMismatch)
        );
        assert_eq!(state.apply(None), expected.apply(None));
    }
}

#[test]
fn name_states_keep_ordinal_abbreviation_domain_and_first_day_order() {
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .with_date_format(DateTextFormat::Iso)
        .with_first_day_of_week(FirstDayOfWeek::new(2).unwrap());
    assert_eq!(
        evaluator.apply_expression_calendar_name(
            TextFunction::MonthName,
            &[number(1), ExpressionValue::Empty]
        ),
        Ok(Value::Text("January".into()))
    );
    assert_eq!(
        evaluator.apply_expression_calendar_name(
            TextFunction::WeekdayName,
            &[number(1), ExpressionValue::Empty, ExpressionValue::Empty]
        ),
        Ok(Value::Text("Monday".into()))
    );
    for (function, arguments, expected) in [
        (
            TextFunction::MonthName,
            vec![ExpressionValue::Empty],
            EvalError::Domain,
        ),
        (
            TextFunction::MonthName,
            vec![number(0), ExpressionValue::Error(11)],
            EvalError::TypeMismatch,
        ),
        (
            TextFunction::MonthName,
            vec![ExpressionValue::Error(11), ExpressionValue::Missing],
            EvalError::TypeMismatch,
        ),
        (
            TextFunction::MonthName,
            vec![number(1), ExpressionValue::Missing],
            EvalError::NonScalarExpression,
        ),
        (
            TextFunction::WeekdayName,
            vec![number(0), number(0), ExpressionValue::Error(11)],
            EvalError::Domain,
        ),
        (
            TextFunction::WeekdayName,
            vec![number(1), number(0), ExpressionValue::Error(11)],
            EvalError::TypeMismatch,
        ),
    ] {
        assert_eq!(
            evaluator.apply_expression_calendar_name(function, &arguments),
            Err(expected)
        );
    }
}

#[test]
fn name_states_keep_output_limits_and_every_successful_cancellation_checkpoint() {
    for function in [TextFunction::MonthName, TextFunction::WeekdayName] {
        let calls = Cell::new(0);
        let invoke = |stop, bytes| {
            calls.set(0);
            let cancelled = || {
                calls.set(calls.get() + 1);
                calls.get() == stop
            };
            let mut budget = ResourceBudget::new(bytes, 0, 1000);
            Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_date_format(DateTextFormat::Iso)
                .with_first_day_of_week(FirstDayOfWeek::new(1).unwrap())
                .with_cancellation(&cancelled)
                .apply_expression_calendar_name(function, &[number(1), ExpressionValue::Empty])
        };
        assert!(invoke(usize::MAX, 1000).is_ok());
        let total = calls.get();
        for stop in 1..=total {
            assert_eq!(invoke(stop, 1000), Err(EvalError::Cancelled));
        }
        assert!(matches!(invoke(usize::MAX, 1), Err(EvalError::Limit(_))));
    }
}

#[test]
fn text_controls_share_exact_conversion_in_materialized_and_expression_apis() {
    for (format, function, controls, expected) in [
        (
            NumberFormat::EnUs,
            TextFunction::MonthName,
            vec!["10"],
            "October",
        ),
        (
            NumberFormat::EnUs,
            TextFunction::MonthName,
            vec!["2.5000000000000000000000000000000001", "True"],
            "Mar",
        ),
        (
            NumberFormat::DeDe,
            TextFunction::MonthName,
            vec!["2,5", "0"],
            "February",
        ),
        (
            NumberFormat::EnUs,
            TextFunction::WeekdayName,
            vec!["1", "False", "2.5"],
            "Monday",
        ),
        (
            NumberFormat::EnUs,
            TextFunction::WeekdayName,
            vec!["1", "0.00001", "0"],
            "Mon",
        ),
    ] {
        let arguments: Vec<_> = controls
            .into_iter()
            .map(|text| Value::Text(text.into()))
            .collect();
        let states: Vec<_> = arguments
            .iter()
            .cloned()
            .map(ExpressionValue::Value)
            .collect();
        for expression in [false, true] {
            let mut budget = ResourceBudget::new(65536, 0, 10000);
            let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_number_format(format)
                .with_date_format(DateTextFormat::EnUs)
                .with_first_day_of_week(FirstDayOfWeek::new(2).unwrap());
            let result = if expression {
                evaluator.apply_expression_calendar_name(function, &states)
            } else {
                evaluator.apply_text_function(function, &arguments)
            };
            assert_eq!(result, Ok(Value::Text(expected.into())), "{arguments:?}");
        }
    }
}

#[test]
fn text_controls_preserve_validation_order_and_exact_long_overflow() {
    for (function, controls, expected) in [
        (
            TextFunction::MonthName,
            vec!["0", "bad"],
            EvalError::TypeMismatch,
        ),
        (
            TextFunction::WeekdayName,
            vec!["0", "False", "bad"],
            EvalError::Domain,
        ),
        (
            TextFunction::WeekdayName,
            vec!["1", "False", "bad"],
            EvalError::TypeMismatch,
        ),
        (
            TextFunction::MonthName,
            vec!["2147483647.5"],
            EvalError::Overflow,
        ),
        (
            TextFunction::MonthName,
            vec!["2147483647.4999999999999999999999999999"],
            EvalError::Domain,
        ),
        (
            TextFunction::WeekdayName,
            vec!["1", "False", "2147483647.5"],
            EvalError::Overflow,
        ),
    ] {
        let arguments: Vec<_> = controls
            .into_iter()
            .map(|text| Value::Text(text.into()))
            .collect();
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_number_format(NumberFormat::EnUs)
            .with_date_format(DateTextFormat::EnUs);
        assert_eq!(
            evaluator.apply_text_function(function, &arguments),
            Err(expected)
        );
    }

    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .with_date_format(DateTextFormat::EnUs);
    assert_eq!(
        evaluator.apply_text_function(TextFunction::MonthName, &[Value::Text("1".into())]),
        Err(EvalError::NumberFormatRequired)
    );
    assert_eq!(
        evaluator.apply_text_function(
            TextFunction::WeekdayName,
            &[
                Value::Int32(0),
                Value::Boolean(false),
                Value::Text("bad".into())
            ]
        ),
        Err(EvalError::Domain)
    );
}

#[test]
fn text_controls_retain_input_output_budgets_and_all_cancellation_checkpoints() {
    for expression in [false, true] {
        let calls = Cell::new(0);
        let invoke = |stop, bytes, max_text_bytes| {
            calls.set(0);
            let cancelled = || {
                calls.set(calls.get() + 1);
                calls.get() == stop
            };
            let arguments = [
                Value::Text("1.0000".into()),
                Value::Text("False".into()),
                Value::Text("2".into()),
            ];
            let states: Vec<_> = arguments
                .iter()
                .cloned()
                .map(ExpressionValue::Value)
                .collect();
            let mut budget = ResourceBudget::new(bytes, 0, 10000);
            let mut evaluator = Evaluator::new(
                &NoBindings,
                &mut budget,
                EvalLimits {
                    max_text_bytes,
                    ..Default::default()
                },
            )
            .with_number_format(NumberFormat::EnUs)
            .with_date_format(DateTextFormat::EnUs)
            .with_cancellation(&cancelled);
            if expression {
                evaluator.apply_expression_calendar_name(TextFunction::WeekdayName, &states)
            } else {
                evaluator.apply_text_function(TextFunction::WeekdayName, &arguments)
            }
        };
        assert_eq!(
            invoke(usize::MAX, 65536, 100),
            Ok(Value::Text("Monday".into()))
        );
        let total = calls.get();
        for stop in 1..=total {
            assert_eq!(
                invoke(stop, 65536, 100),
                Err(EvalError::Cancelled),
                "checkpoint {stop}"
            );
        }
        assert!(matches!(
            invoke(usize::MAX, 1, 100),
            Err(EvalError::Limit(_))
        ));
        assert_eq!(invoke(usize::MAX, 65536, 5), Err(EvalError::TextLimit));
    }
}

use super::*;
use crate::{
    evaluation::{EvalLimits, NoBindings},
    functions::{conversions::text::NumberFormat, text::cstr::DateTextFormat},
};
use std::cell::Cell;
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Currency, Decimal, LegacyDateTime},
};

#[test]
fn strcomp_formats_scalar_operands_under_explicit_policies_and_keeps_null_precedence() {
    let mut budget = ResourceBudget::new(65536, 0, 65536);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .with_number_format(NumberFormat::DeDe)
        .with_date_format(DateTextFormat::Iso);
    for (value, text) in [
        (Value::Int64(9007199254740993), "9007199254740993"),
        (Value::Boolean(true), "True"),
        (Value::Currency(Currency::from_scaled(12500)), "1,25"),
        (Value::Decimal(Decimal::new(12345, 3).unwrap()), "12,345"),
        (
            Value::DateTime(LegacyDateTime::from_ole_days(2.0).unwrap()),
            "1900-01-01",
        ),
    ] {
        assert_eq!(
            evaluator.apply_text_function(
                TextFunction::StrComp,
                &[value.clone(), Value::Text(text.into()), Value::Int16(0)]
            ),
            Ok(Value::Int16(0))
        );
        assert_eq!(
            evaluator.apply_text_function(
                TextFunction::StrComp,
                &[Value::Text(text.into()), value, Value::Int16(0)]
            ),
            Ok(Value::Int16(0))
        );
    }
    let mut budget = ResourceBudget::new(0, 0, 65536);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    assert_eq!(
        evaluator.apply_text_function(
            TextFunction::StrComp,
            &[Value::Int64(123), Value::Null, Value::Int16(0)]
        ),
        Ok(Value::Null)
    );
    assert_eq!(
        evaluator.apply_text_function(
            TextFunction::StrComp,
            &[Value::Int64(123), Value::Null, Value::Int16(3)]
        ),
        Err(EvalError::Domain)
    );
    assert_eq!(
        evaluator.apply_text_function(
            TextFunction::StrComp,
            &[
                Value::Int64(123),
                Value::Text("123".into()),
                Value::Int16(0)
            ]
        ),
        Err(EvalError::NumberFormatRequired)
    );
}

#[test]
fn strcomp_operand_materialization_is_bounded_and_checks_all_cancellation_points() {
    let arguments = [
        Value::Int64(1234),
        Value::Text("1234".into()),
        Value::Int16(0),
    ];
    let mut budget = ResourceBudget::new(3, 0, 65536);
    assert!(matches!(
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_number_format(NumberFormat::EnUs)
            .apply_text_function(TextFunction::StrComp, &arguments),
        Err(EvalError::Limit(_))
    ));
    let calls = Cell::new(0);
    let invoke = |stop| {
        calls.set(0);
        let cancelled = || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        };
        let mut budget = ResourceBudget::new(65536, 0, 65536);
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_number_format(NumberFormat::EnUs)
            .with_cancellation(&cancelled)
            .apply_text_function(TextFunction::StrComp, &arguments)
    };
    assert_eq!(invoke(usize::MAX), Ok(Value::Int16(0)));
    let total = calls.get();
    for stop in 1..=total {
        assert_eq!(invoke(stop), Err(EvalError::Cancelled));
    }
}

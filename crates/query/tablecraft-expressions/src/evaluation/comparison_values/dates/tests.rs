use super::*;
use crate::{
    comparison::{self, Comparison},
    evaluation::{EvalLimits, NoBindings},
    functions::{
        conversions::text::NumberFormat,
        dates::{
            TwoDigitYearWindow,
            parsing::{DateTextFormat, ReferenceYear},
        },
    },
    logic::Truth,
};
use std::cell::Cell;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{ExtendedDateTime, LegacyDateTime},
};

fn date(serial: f64) -> Value {
    Value::DateTime(LegacyDateTime::from_ole_days(serial).unwrap())
}

#[test]
fn legacy_date_text_uses_date_syntax_and_optional_numeric_fallback() {
    let mut budget = ResourceBudget::new(65536, 0, 65536);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .with_date_format(DateTextFormat::Iso);
    assert_eq!(
        evaluator.compare_values(
            Comparison::Equal,
            &date(45658.5),
            &Value::Text("2025-01-01 12:00:00".into())
        ),
        Ok(Truth::True)
    );
    assert_eq!(
        evaluator.compare_values(
            Comparison::Less,
            &Value::Text("2025-01-01 11:59:59".into()),
            &date(45658.5)
        ),
        Ok(Truth::True)
    );
    assert_eq!(
        evaluator.compare_values(Comparison::Equal, &date(0.25), &Value::Text("-0,25".into())),
        Err(EvalError::TypeMismatch)
    );
    let mut evaluator = evaluator.with_number_format(NumberFormat::DeDe);
    assert_eq!(
        evaluator.compare_values(Comparison::Equal, &date(0.25), &Value::Text("-0,25".into())),
        Ok(Truth::True)
    );
    assert_eq!(
        comparison::compare(
            Comparison::Equal,
            &date(45658.5),
            &Value::Text("2025-01-01 12:00:00".into()),
            None
        ),
        Err(EvalError::TypeMismatch)
    );
}

#[test]
fn year_controls_and_extended_date_refusal_are_execution_local() {
    let mut budget = ResourceBudget::new(65536, 0, 65536);
    let moment = Value::DateTime(
        LegacyDateTime::from_calendar(jiff::civil::Date::constant(2030, 1, 1).at(0, 0, 0, 0))
            .unwrap(),
    );
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .with_date_format(DateTextFormat::EnUs);
    assert_eq!(
        evaluator.compare_values(Comparison::Equal, &moment, &Value::Text("01/01/30".into())),
        Ok(Truth::False)
    );
    let mut evaluator = evaluator.with_year_window(TwoDigitYearWindow::new(2099).unwrap());
    assert_eq!(
        evaluator.compare_values(Comparison::Equal, &moment, &Value::Text("01/01/30".into())),
        Ok(Truth::True)
    );
    assert_eq!(
        evaluator.compare_values(Comparison::Equal, &moment, &Value::Text("1/1".into())),
        Err(EvalError::ReferenceYearRequired)
    );
    let mut evaluator = evaluator.with_reference_year(ReferenceYear::new(2030).unwrap());
    assert_eq!(
        evaluator.compare_values(Comparison::Equal, &moment, &Value::Text("1/1".into())),
        Ok(Truth::True)
    );
    let extended = Value::DateTimeExtended(ExtendedDateTime::new(2030, 1, 1, 0, 0, 0, 1).unwrap());
    assert_eq!(
        evaluator.compare_values(
            Comparison::Equal,
            &extended,
            &Value::Text("01/01/2030".into())
        ),
        Err(EvalError::TypeMismatch)
    );
}

#[test]
fn missing_policies_nulls_limits_and_conversion_cancellation_remain_explicit() {
    let mut budget = ResourceBudget::new(65536, 0, 65536);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    assert_eq!(
        evaluator.compare_values(
            Comparison::Equal,
            &date(0.0),
            &Value::Text("1899-12-30".into())
        ),
        Err(EvalError::DateFormatRequired)
    );
    assert_eq!(
        evaluator.compare_values(Comparison::Equal, &Value::Null, &Value::Text("bad".into())),
        Ok(Truth::Unknown)
    );
    let mut budget = ResourceBudget::new(65536, 0, 65536);
    assert_eq!(
        Evaluator::new(
            &NoBindings,
            &mut budget,
            EvalLimits {
                max_text_bytes: 3,
                ..Default::default()
            }
        )
        .with_date_format(DateTextFormat::Iso)
        .compare_values(
            Comparison::Equal,
            &date(0.0),
            &Value::Text("1899-12-30".into())
        ),
        Err(EvalError::TextLimit)
    );
    let mut budget = ResourceBudget::new(2, 0, 65536);
    let result = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .with_date_format(DateTextFormat::Iso)
        .with_number_format(NumberFormat::EnUs)
        .compare_values(Comparison::Equal, &date(0.0), &Value::Text("0.25".into()));
    assert!(matches!(result, Err(EvalError::Limit(error)) if error.resource == Resource::Bytes));
    let calls = Cell::new(0);
    let invoke = |stop| {
        let cancel = || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        };
        let mut budget = ResourceBudget::new(65536, 0, 65536);
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_date_format(DateTextFormat::Iso)
            .with_cancellation(&cancel)
            .compare_values(
                Comparison::Equal,
                &date(45658.5),
                &Value::Text("2025-01-01 12:00:00".into()),
            )
    };
    assert_eq!(invoke(usize::MAX), Ok(Truth::True));
    let total = calls.get();
    for stop in 1..=total {
        calls.set(0);
        assert_eq!(invoke(stop), Err(EvalError::Cancelled), "checkpoint {stop}");
    }
}

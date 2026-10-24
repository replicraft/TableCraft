use super::*;
use crate::{
    evaluation::{EvalLimits, NoBindings},
    functions::{
        conversions::text::NumberFormat,
        dates::{DateAdd, DateDiff, DatePart, WeekContext, parsing::DateTextFormat},
    },
};
use std::cell::Cell;
use tablecraft_core::{
    limits::ResourceBudget,
    values::{ExpressionString, ExtendedDateTime, LegacyDateTime},
};

fn retained(text: &str) -> ExpressionValue {
    let mut bytes = vec![0, 216];
    bytes.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
    let end = bytes.len();
    bytes.push(255);
    ExpressionValue::StringBytes(ExpressionString::from_owned(bytes).select(2..end).unwrap())
}

#[test]
fn all_ten_date_calls_accept_retained_sources_and_controls_under_existing_policies() {
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .with_number_format(NumberFormat::EnUs)
        .with_date_format(DateTextFormat::Iso);
    let date = retained("1900-01-01 12:34:56");
    for (function, expected) in [
        (DateFunction::Year, 1900),
        (DateFunction::Month, 1),
        (DateFunction::Day, 1),
        (DateFunction::Hour, 12),
        (DateFunction::Minute, 34),
        (DateFunction::Second, 56),
    ] {
        assert_eq!(
            evaluator.apply_expression_date_function(function, &date),
            Ok(Value::Int16(expected))
        );
    }
    assert_eq!(
        evaluator.apply_expression_weekday(&date, Some(&retained("2"))),
        Ok(Value::Int16(1))
    );
    let first = retained("1900-01-01 12:00:00");
    assert_eq!(
        evaluator.apply_expression_date_add(&retained("d"), &retained("1.5"), &first),
        Ok(Value::DateTime(LegacyDateTime::from_ole_days(4.5).unwrap()))
    );
    assert_eq!(
        evaluator.apply_expression_date_part(
            &retained("q"),
            &first,
            Some(&retained("1")),
            Some(&retained("1"))
        ),
        Ok(Value::Int16(1))
    );
    assert_eq!(
        evaluator.apply_expression_date_diff(
            &retained("d"),
            &first,
            &retained("1900-01-03 12:00:00"),
            Some(&retained("1")),
            Some(&retained("1"))
        ),
        Ok(Value::Int32(2))
    );
}

fn malformed() -> ExpressionValue {
    ExpressionValue::StringBytes(
        ExpressionString::from_owned(vec![b'7', 0, 0, 216, b'8', 0])
            .select(2..4)
            .unwrap(),
    )
}

#[test]
fn retained_inputs_preserve_progressive_control_and_source_failures() {
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .with_number_format(NumberFormat::EnUs)
        .with_date_format(DateTextFormat::Iso);
    let bad = malformed();
    let one = ExpressionValue::Value(Value::Int32(1));
    let null = ExpressionValue::Value(Value::Null);
    assert_eq!(
        evaluator.apply_expression_date_add(&retained("bad"), &bad, &bad),
        Err(EvalError::InvalidDateInterval)
    );
    assert_eq!(
        evaluator.apply_expression_date_add(
            &retained("d"),
            &ExpressionValue::Value(Value::Int64(i64::MAX)),
            &bad
        ),
        Err(EvalError::Overflow)
    );
    assert_eq!(
        evaluator.apply_expression_date_add(&retained("d"), &ExpressionValue::Error(7), &bad),
        Err(EvalError::TypeMismatch)
    );
    assert_eq!(
        evaluator.apply_expression_date_add(&bad, &one, &null),
        Err(EvalError::UnrepresentableText)
    );
    for (day, week, expected) in [
        (retained("8"), bad.clone(), EvalError::Domain),
        (
            retained("0"),
            bad.clone(),
            EvalError::CalendarPolicyRequired,
        ),
        (retained("1"), bad.clone(), EvalError::UnrepresentableText),
        (retained("1"), retained("4"), EvalError::Domain),
    ] {
        assert_eq!(
            evaluator.apply_expression_date_part(&retained("d"), &bad, Some(&day), Some(&week)),
            Err(expected)
        );
        assert_eq!(
            evaluator.apply_expression_date_diff(
                &retained("d"),
                &bad,
                &bad,
                Some(&day),
                Some(&week)
            ),
            Err(expected)
        );
    }
    assert_eq!(
        evaluator.apply_expression_date_part(&retained("bad"), &bad, Some(&bad), Some(&bad)),
        Err(EvalError::InvalidDateInterval)
    );
    assert_eq!(
        evaluator.apply_expression_date_diff(
            &retained("d"),
            &retained("invalid"),
            &bad,
            None,
            None
        ),
        Err(EvalError::TypeMismatch)
    );
    assert_eq!(
        evaluator.apply_expression_date_diff(&retained("d"), &null, &bad, None, None),
        Err(EvalError::UnrepresentableText)
    );
    assert_eq!(
        evaluator.apply_expression_weekday(&bad, Some(&retained("8"))),
        Err(EvalError::Domain)
    );
    assert_eq!(
        evaluator.apply_expression_weekday(&bad, Some(&retained("0"))),
        Err(EvalError::CalendarPolicyRequired)
    );
    assert_eq!(
        evaluator.apply_expression_weekday(&bad, Some(&retained("1"))),
        Err(EvalError::UnrepresentableText)
    );

    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    assert_eq!(
        evaluator.apply_expression_date_add(&retained("d"), &retained("1"), &bad),
        Err(EvalError::NumberFormatRequired)
    );
    assert_eq!(
        evaluator.apply_expression_date_add(&retained("bad"), &retained("1"), &bad),
        Err(EvalError::InvalidDateInterval)
    );
}

#[test]
fn existing_dates_keep_their_carrier_and_extended_precision() {
    let extended = ExtendedDateTime::new(1, 1, 1, 12, 34, 56, 1234567).unwrap();
    let date = ExpressionValue::Value(Value::DateTimeExtended(extended));
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .with_number_format(NumberFormat::EnUs);
    assert_eq!(
        evaluator.apply_expression_date_function(DateFunction::Year, &date),
        Ok(Value::Int16(1))
    );
    assert_eq!(
        evaluator.apply_expression_date_part(&retained("yyyy"), &date, None, None),
        Ok(Value::Int16(1))
    );
    assert_eq!(
        evaluator.apply_expression_date_add(&retained("s"), &retained("1"), &date),
        Ok(Value::DateTimeExtended(
            ExtendedDateTime::new(1, 1, 1, 12, 34, 57, 1234567).unwrap()
        ))
    );
    assert_eq!(
        evaluator.apply_expression_date_diff(
            &retained("d"),
            &date,
            &ExpressionValue::Value(Value::DateTimeExtended(
                ExtendedDateTime::new(1, 1, 3, 12, 34, 56, 1234567).unwrap()
            )),
            None,
            None
        ),
        Ok(Value::Int32(2))
    );
    assert_eq!(
        date,
        ExpressionValue::Value(Value::DateTimeExtended(extended))
    );
}

#[derive(Clone, Copy)]
enum Operation {
    Component(DateFunction),
    Weekday,
    Add,
    Part,
    Diff,
}

fn apply(evaluator: &mut Evaluator<'_>, operation: Operation) -> Result<Value, EvalError> {
    let date = retained("1900-01-01 12:34:56");
    match operation {
        Operation::Component(function) => evaluator.apply_expression_date_function(function, &date),
        Operation::Weekday => evaluator.apply_expression_weekday(&date, Some(&retained("1"))),
        Operation::Add => {
            evaluator.apply_expression_date_add(&retained("d"), &retained("1.5"), &date)
        }
        Operation::Part => evaluator.apply_expression_date_part(
            &retained("q"),
            &date,
            Some(&retained("1")),
            Some(&retained("1")),
        ),
        Operation::Diff => evaluator.apply_expression_date_diff(
            &retained("d"),
            &date,
            &retained("1900-01-03 12:34:56"),
            Some(&retained("1")),
            Some(&retained("1")),
        ),
    }
}

#[test]
fn every_retained_date_operation_meters_publication_conversion_and_cancellation() {
    let operations = [
        Operation::Component(DateFunction::Year),
        Operation::Component(DateFunction::Month),
        Operation::Component(DateFunction::Day),
        Operation::Component(DateFunction::Hour),
        Operation::Component(DateFunction::Minute),
        Operation::Component(DateFunction::Second),
        Operation::Weekday,
        Operation::Add,
        Operation::Part,
        Operation::Diff,
    ];
    for operation in operations {
        let calls = Cell::new(0);
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        apply(
            &mut Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_number_format(NumberFormat::EnUs)
                .with_date_format(DateTextFormat::Iso)
                .with_cancellation(&|| {
                    calls.set(calls.get() + 1);
                    false
                }),
            operation,
        )
        .unwrap();
        for stop in 1..=calls.get() {
            let observed = Cell::new(0);
            let mut budget = ResourceBudget::new(65536, 0, 10000);
            assert_eq!(
                apply(
                    &mut Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                        .with_number_format(NumberFormat::EnUs)
                        .with_date_format(DateTextFormat::Iso)
                        .with_cancellation(&|| {
                            observed.set(observed.get() + 1);
                            observed.get() >= stop
                        }),
                    operation
                ),
                Err(EvalError::Cancelled)
            );
        }
        for (bytes, steps) in [(0, 10000), (65536, 0)] {
            let mut budget = ResourceBudget::new(bytes, 0, steps);
            assert!(matches!(
                apply(
                    &mut Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                        .with_number_format(NumberFormat::EnUs)
                        .with_date_format(DateTextFormat::Iso),
                    operation
                ),
                Err(EvalError::Limit(_))
            ));
        }
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        assert_eq!(
            apply(
                &mut Evaluator::new(
                    &NoBindings,
                    &mut budget,
                    EvalLimits {
                        max_text_bytes: 1,
                        ..Default::default()
                    }
                )
                .with_number_format(NumberFormat::EnUs)
                .with_date_format(DateTextFormat::Iso),
                operation
            ),
            Err(EvalError::TextLimit)
        );
    }
}

#[test]
fn prepared_control_adapters_convert_each_reached_argument_once() {
    let calls = Cell::new(0);
    let interval = Value::Text("d".into());
    DateAdd::prepare_with_count(&interval, || {
        calls.set(calls.get() + 1);
        Ok(1)
    })
    .unwrap();
    assert_eq!(calls.get(), 1);
    calls.set(0);
    assert!(matches!(
        DateAdd::prepare_with_count(&Value::Text("bad".into()), || {
            calls.set(calls.get() + 1);
            Ok(1)
        }),
        Err(EvalError::InvalidDateInterval)
    ));
    assert_eq!(calls.get(), 0);

    for part in [false, true] {
        for (label, first_day, expected_calls, expected_error) in [
            ("d", 1, 2, None),
            ("d", 8, 1, Some(EvalError::Domain)),
            ("bad", 1, 0, Some(EvalError::InvalidDateInterval)),
        ] {
            let calls = Cell::new(0);
            let conversions = Cell::new(0);
            let values = [Value::Int32(first_day), Value::Int32(1)];
            let interval = Value::Text(label.into());
            let option = |index| {
                assert_eq!(index, calls.get());
                calls.set(calls.get() + 1);
                Ok(Some(&values[index]))
            };
            let convert = |value: &Value| {
                conversions.set(conversions.get() + 1);
                let Value::Int32(value) = value else {
                    panic!("test control must be Long");
                };
                Ok(*value)
            };
            let error = if part {
                DatePart::prepare_with_options(&interval, option, WeekContext::default(), convert)
                    .err()
            } else {
                DateDiff::prepare_with_options(&interval, option, WeekContext::default(), convert)
                    .err()
            };
            assert_eq!(error, expected_error);
            assert_eq!(calls.get(), expected_calls);
            assert_eq!(conversions.get(), expected_calls);
        }
    }
}

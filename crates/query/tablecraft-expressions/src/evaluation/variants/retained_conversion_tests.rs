use super::*;
use crate::{
    evaluation::{EvalLimits, NoBindings},
    functions::{
        conversions::text::NumberFormat,
        dates::parsing::{DateConversion, DateTextFormat},
    },
};
use std::cell::Cell;
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Currency, Decimal, Double, LegacyDateTime, Single},
};

#[derive(Clone, Copy)]
enum Target {
    Numeric(Conversion),
    Date(DateConversion),
}

fn convert(
    evaluator: &mut Evaluator<'_>,
    target: Target,
    value: &ExpressionValue,
) -> Result<Value, EvalError> {
    match target {
        Target::Numeric(target) => evaluator.apply_expression_conversion(target, value),
        Target::Date(target) => evaluator.apply_expression_date_conversion(target, value),
    }
}

fn retained(text: &str) -> ExpressionValue {
    // Discarded backing storage contains an unpaired high surrogate and an odd
    // trailing byte. Only the selected numeric/date units belong to the input.
    let mut bytes = vec![0, 216];
    bytes.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
    let end = bytes.len();
    bytes.push(255);
    ExpressionValue::StringBytes(ExpressionString::from_owned(bytes).select(2..end).unwrap())
}

fn targets() -> Vec<(Target, &'static str, Value)> {
    vec![
        (Target::Numeric(Conversion::Byte), "2.5", Value::Byte(2)),
        (Target::Numeric(Conversion::Integer), "2.5", Value::Int16(2)),
        (Target::Numeric(Conversion::Long), "2.5", Value::Int32(2)),
        (
            Target::Numeric(Conversion::LongLong),
            "2.5",
            Value::Int64(2),
        ),
        (
            Target::Numeric(Conversion::Boolean),
            "2.5",
            Value::Boolean(true),
        ),
        (
            Target::Numeric(Conversion::Single),
            "2.5",
            Value::Single(Single::from_float(2.5)),
        ),
        (
            Target::Numeric(Conversion::Double),
            "2.5",
            Value::Double(Double::from_float(2.5)),
        ),
        (
            Target::Numeric(Conversion::Currency),
            "2.5",
            Value::Currency(Currency::from_scaled(25000)),
        ),
        (
            Target::Numeric(Conversion::Decimal),
            "2.5",
            Value::Decimal(Decimal::new(25, 1).unwrap()),
        ),
        (
            Target::Date(DateConversion::CDate),
            "1900-01-01 12:00:00",
            Value::DateTime(LegacyDateTime::from_ole_days(2.5).unwrap()),
        ),
        (
            Target::Date(DateConversion::CVDate),
            "1900-01-01 12:00:00",
            Value::DateTime(LegacyDateTime::from_ole_days(2.5).unwrap()),
        ),
        (
            Target::Date(DateConversion::DateValue),
            "1900-01-01 12:00:00",
            Value::DateTime(LegacyDateTime::from_ole_days(2.0).unwrap()),
        ),
        (
            Target::Date(DateConversion::TimeValue),
            "1900-01-01 12:00:00",
            Value::DateTime(LegacyDateTime::from_ole_days(0.5).unwrap()),
        ),
    ]
}

#[test]
fn retained_and_materialized_text_use_the_same_thirteen_conversion_kernels() {
    for (target, text, expected) in targets() {
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_number_format(NumberFormat::EnUs)
            .with_date_format(DateTextFormat::Iso);
        assert_eq!(
            convert(
                &mut evaluator,
                target,
                &ExpressionValue::Value(Value::Text(text.into()))
            ),
            Ok(expected.clone())
        );
        assert_eq!(
            convert(&mut evaluator, target, &retained(text)),
            Ok(expected)
        );
    }
}

#[test]
fn retained_numeric_boundaries_remain_exact_and_policies_do_not_appear_implicitly() {
    for (target, text, expected) in [
        (
            Conversion::LongLong,
            "9223372036854775807",
            Value::Int64(i64::MAX),
        ),
        (
            Conversion::LongLong,
            "-9223372036854775808",
            Value::Int64(i64::MIN),
        ),
        (
            Conversion::Currency,
            "922337203685477.5807",
            Value::Currency(Currency::from_scaled(i64::MAX)),
        ),
        (
            Conversion::Decimal,
            "79228162514264337593543950335",
            Value::Decimal(Decimal::new(79228162514264337593543950335, 0).unwrap()),
        ),
    ] {
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
        assert_eq!(
            evaluator.apply_expression_conversion(target, &retained(text)),
            Err(EvalError::NumberFormatRequired)
        );
        assert_eq!(
            evaluator
                .with_number_format(NumberFormat::EnUs)
                .apply_expression_conversion(target, &retained(text)),
            Ok(expected)
        );
    }
    for target in [
        DateConversion::CDate,
        DateConversion::CVDate,
        DateConversion::DateValue,
        DateConversion::TimeValue,
    ] {
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .apply_expression_date_conversion(target, &retained("1900-01-01 12:00:00")),
            Err(EvalError::DateFormatRequired)
        );
    }
}

#[test]
fn every_retained_conversion_preserves_raw_refusals_and_shared_resource_controls() {
    for (target, text, _) in targets() {
        for bytes in [
            ExpressionString::from_owned(vec![b'7', 0, 0, 216, b'8', 0])
                .select(2..4)
                .unwrap(),
            ExpressionString::from_owned(vec![b'1', 0, b'2', 0])
                .select(1..4)
                .unwrap(),
        ] {
            let mut budget = ResourceBudget::new(65536, 0, 10000);
            let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_number_format(NumberFormat::EnUs)
                .with_date_format(DateTextFormat::Iso);
            let value = ExpressionValue::StringBytes(bytes);
            assert_eq!(
                convert(&mut evaluator, target, &value),
                Err(EvalError::UnrepresentableText)
            );
            let mut budget = ResourceBudget::new(65536, 0, 10000);
            assert_eq!(
                convert(
                    &mut Evaluator::new(&NoBindings, &mut budget, EvalLimits::default()),
                    target,
                    &value,
                ),
                Err(EvalError::UnrepresentableText)
            );
            let mut budget = ResourceBudget::new(65536, 0, 10000);
            assert_eq!(
                convert(
                    &mut Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                        .with_cancellation(&|| true),
                    target,
                    &value,
                ),
                Err(EvalError::Cancelled)
            );
        }
        let calls = Cell::new(0);
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        let value = retained(text);
        convert(
            &mut Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_number_format(NumberFormat::EnUs)
                .with_date_format(DateTextFormat::Iso)
                .with_cancellation(&|| {
                    calls.set(calls.get() + 1);
                    false
                }),
            target,
            &value,
        )
        .unwrap();
        for stop in 1..=calls.get() {
            let observed = Cell::new(0);
            let mut budget = ResourceBudget::new(65536, 0, 10000);
            assert_eq!(
                convert(
                    &mut Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                        .with_number_format(NumberFormat::EnUs)
                        .with_date_format(DateTextFormat::Iso)
                        .with_cancellation(&|| {
                            observed.set(observed.get() + 1);
                            observed.get() >= stop
                        }),
                    target,
                    &value,
                ),
                Err(EvalError::Cancelled)
            );
        }
        for (bytes, steps) in [(0, 10000), (65536, 0)] {
            let mut budget = ResourceBudget::new(bytes, 0, steps);
            assert!(matches!(
                convert(
                    &mut Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                        .with_number_format(NumberFormat::EnUs)
                        .with_date_format(DateTextFormat::Iso),
                    target,
                    &value,
                ),
                Err(EvalError::Limit(_))
            ));
        }
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        assert_eq!(
            convert(
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
                target,
                &value,
            ),
            Err(EvalError::TextLimit)
        );
    }
}

#[test]
fn new_string_admission_does_not_change_empty_null_missing_or_error_exceptions() {
    for (target, _, _) in targets() {
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
        let (empty, null, missing, error) = match target {
            Target::Numeric(conversion) => (
                conversion.apply(&Value::Int16(0)),
                Err(EvalError::InvalidNullConversion),
                Err(EvalError::TypeMismatch),
                Err(if conversion == Conversion::Decimal {
                    EvalError::TypeMismatch
                } else {
                    EvalError::UnsupportedExpression
                }),
            ),
            Target::Date(DateConversion::CDate | DateConversion::CVDate) => (
                Ok(Value::DateTime(LegacyDateTime::from_ole_days(0.0).unwrap())),
                Err(EvalError::InvalidNullConversion),
                Err(EvalError::NonScalarExpression),
                Err(EvalError::TypeMismatch),
            ),
            Target::Date(DateConversion::DateValue | DateConversion::TimeValue) => (
                Err(EvalError::TypeMismatch),
                Ok(Value::Null),
                Err(EvalError::NonScalarExpression),
                Err(EvalError::TypeMismatch),
            ),
        };
        for (input, expected) in [
            (ExpressionValue::Empty, empty),
            (ExpressionValue::Value(Value::Null), null),
            (ExpressionValue::Missing, missing),
            (ExpressionValue::Error(65536), error),
        ] {
            assert_eq!(convert(&mut evaluator, target, &input), expected);
        }
    }
}

use super::*;
use crate::{
    evaluation::{EvalLimits, NoBindings},
    functions::{conversions::text::NumberFormat, dates::TwoDigitYearWindow},
};
use jiff::civil::date;
use std::cell::Cell;
use tablecraft_core::{
    limits::ResourceBudget,
    values::{ExpressionString, LegacyDateTime},
};

fn retained(text: &str) -> ExpressionValue {
    let mut bytes = vec![0, 216];
    bytes.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
    let end = bytes.len();
    bytes.push(255);
    ExpressionValue::StringBytes(ExpressionString::from_owned(bytes).select(2..end).unwrap())
}

fn malformed() -> ExpressionValue {
    ExpressionValue::StringBytes(ExpressionString::from_owned(vec![0, 216]))
}

#[test]
fn constructors_admit_only_active_strings_under_explicit_number_and_year_policies() {
    for format in [NumberFormat::EnUs, NumberFormat::DeDe] {
        for (function, components, expected) in [
            (
                DateConstructor::DateSerial,
                ["29", "2.5", "28.5"],
                LegacyDateTime::from_calendar(date(1929, 2, 28).at(0, 0, 0, 0)).unwrap(),
            ),
            (
                DateConstructor::TimeSerial,
                ["-1", "30.5", "0"],
                LegacyDateTime::from_ole_days(-1.0 / 48.0).unwrap(),
            ),
        ] {
            let strings = components.map(|text| {
                if format == NumberFormat::DeDe {
                    text.replace('.', ",")
                } else {
                    text.to_owned()
                }
            });
            let scalar = strings.clone().map(Value::Text);
            let retained = strings.map(|text| retained(&text));
            let mut budget = ResourceBudget::new(65536, 0, 10000);
            let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_number_format(format)
                .with_year_window(TwoDigitYearWindow::new(1999).unwrap());
            assert_eq!(
                evaluator.apply_date_constructor(function, &scalar),
                Ok(Value::DateTime(expected))
            );

            assert_eq!(
                evaluator.apply_expression_date_constructor(function, &retained),
                Ok(Value::DateTime(expected))
            );
        }
    }
}

#[test]
fn component_admission_and_integer_conversion_keep_written_failure_order() {
    for function in [DateConstructor::DateSerial, DateConstructor::TimeSerial] {
        for (values, error) in [
            (vec![malformed()], EvalError::UnsupportedExpression),
            (
                vec![retained("32768"), malformed(), malformed()],
                EvalError::Overflow,
            ),
            (
                vec![retained("invalid"), malformed(), malformed()],
                EvalError::TypeMismatch,
            ),
            (
                vec![retained("1"), malformed(), retained("1")],
                EvalError::UnrepresentableText,
            ),
            (
                vec![
                    ExpressionValue::Value(Value::Null),
                    malformed(),
                    malformed(),
                ],
                EvalError::InvalidNullConversion,
            ),
            (
                vec![ExpressionValue::Error(1), malformed(), malformed()],
                EvalError::TypeMismatch,
            ),
            (
                vec![ExpressionValue::Missing, malformed(), malformed()],
                EvalError::NonScalarExpression,
            ),
            (
                vec![
                    retained("1"),
                    ExpressionValue::StringBytes(ExpressionString::from_owned(vec![49])),
                    malformed(),
                ],
                EvalError::UnrepresentableText,
            ),
        ] {
            let mut budget = ResourceBudget::new(65536, 0, 10000);
            let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_number_format(NumberFormat::EnUs);
            assert_eq!(
                evaluator.apply_expression_date_constructor(function, &values),
                Err(error)
            );
        }

        let mut budget = ResourceBudget::new(65536, 0, 10000);
        let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
        assert_eq!(
            evaluator.apply_expression_date_constructor(
                function,
                &[retained("1"), malformed(), malformed()]
            ),
            Err(EvalError::NumberFormatRequired)
        );
    }
}

#[test]
fn retained_constructor_publication_checks_every_cancellation_and_resource_boundary() {
    for function in [DateConstructor::DateSerial, DateConstructor::TimeSerial] {
        let values = [retained("24"), retained("1"), retained("1")];
        let checks = Cell::new(0);
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_number_format(NumberFormat::EnUs)
            .with_cancellation(&|| {
                checks.set(checks.get() + 1);
                false
            })
            .apply_expression_date_constructor(function, &values)
            .unwrap();

        for stop in 1..=checks.get() {
            let calls = Cell::new(0);
            let mut budget = ResourceBudget::new(65536, 0, 10000);
            assert_eq!(
                Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                    .with_number_format(NumberFormat::EnUs)
                    .with_cancellation(&|| {
                        calls.set(calls.get() + 1);
                        calls.get() >= stop
                    })
                    .apply_expression_date_constructor(function, &values),
                Err(EvalError::Cancelled)
            );
        }

        for mut budget in [
            ResourceBudget::new(0, 0, 10000),
            ResourceBudget::new(65536, 0, 0),
        ] {
            assert!(matches!(
                Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                    .with_number_format(NumberFormat::EnUs)
                    .apply_expression_date_constructor(function, &values),
                Err(EvalError::Limit(_))
            ));
        }

        let mut budget = ResourceBudget::new(65536, 0, 10000);
        assert_eq!(
            Evaluator::new(
                &NoBindings,
                &mut budget,
                EvalLimits {
                    max_text_bytes: 1,
                    ..EvalLimits::default()
                }
            )
            .with_number_format(NumberFormat::EnUs)
            .apply_expression_date_constructor(function, &values),
            Err(EvalError::TextLimit)
        );
    }
}

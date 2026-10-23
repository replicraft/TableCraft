use super::*;
use crate::{
    evaluation::{EvalLimits, NoBindings},
    functions::text::cstr::DateTextFormat,
};
use std::cell::Cell;
use tablecraft_core::{
    limits::ResourceBudget,
    values::{ExtendedDateTime, LegacyDateTime},
};
use tablecraft_sql::{parse_select, select::Projection};

#[test]
fn str_consumes_standard_error_empty_and_legacy_dates_under_explicit_policies() {
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    for (code, expected) in [(0, "Error 0"), (2001, "Error 2001"), (65535, "Error 65535")] {
        assert_eq!(
            evaluator.apply_expression_str(&ExpressionValue::Error(code)),
            Ok(Value::Text(expected.into()))
        );
    }
    assert_eq!(
        evaluator.apply_expression_str(&ExpressionValue::Empty),
        Ok(Value::Text(" 0".into()))
    );
    assert_eq!(
        evaluator.apply_expression_str(&ExpressionValue::Missing),
        Err(EvalError::NonScalarExpression)
    );
    assert_eq!(
        evaluator.apply_expression_str(&ExpressionValue::Error(65536)),
        Err(EvalError::UnsupportedExpression)
    );
    let date = Value::DateTime(LegacyDateTime::from_ole_days(2.0).unwrap());
    assert_eq!(
        evaluator.apply_text_function(TextFunction::Str, std::slice::from_ref(&date)),
        Err(EvalError::DateFormatRequired)
    );
    let mut evaluator = evaluator.with_date_format(DateTextFormat::Iso);
    assert_eq!(
        evaluator.apply_text_function(TextFunction::Str, &[date]),
        Ok(Value::Text("1900-01-01".into()))
    );
    let extended =
        Value::DateTimeExtended(ExtendedDateTime::new(2026, 10, 23, 0, 0, 0, 0).unwrap());
    assert_eq!(
        evaluator.apply_text_function(TextFunction::Str, &[extended]),
        Err(EvalError::TypeMismatch)
    );
    for (sql, error) in [
        (
            "SELECT Str(IIf(True,CVErr(1),1/0))",
            EvalError::DivisionByZero,
        ),
        ("SELECT Str(CVErr(1),1/0)", EvalError::UnsupportedExpression),
    ] {
        let parsed = parse_select(sql, Default::default()).unwrap();
        let Projection::Expression { expression, .. } = &parsed.query().projection[0] else {
            panic!("expression required")
        };
        assert_eq!(evaluator.evaluate(expression), Err(error));
    }
}

#[test]
fn str_expression_state_conversion_shares_text_and_cancellation_limits() {
    for value in [ExpressionValue::Error(2001), ExpressionValue::Empty] {
        let checks = Cell::new(0);
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_cancellation(&|| {
                checks.set(checks.get() + 1);
                false
            })
            .apply_expression_str(&value)
            .unwrap();
        for stop in 1..=checks.get() {
            let calls = Cell::new(0);
            let mut budget = ResourceBudget::new(65536, 0, 10000);
            assert_eq!(
                Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                    .with_cancellation(&|| {
                        calls.set(calls.get() + 1);
                        calls.get() >= stop
                    })
                    .apply_expression_str(&value),
                Err(EvalError::Cancelled)
            );
        }
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        assert_eq!(
            Evaluator::new(
                &NoBindings,
                &mut budget,
                EvalLimits {
                    max_text_bytes: 1,
                    ..Default::default()
                }
            )
            .apply_expression_str(&value),
            Err(EvalError::TextLimit)
        );
    }
}

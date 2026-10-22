use super::*;
use crate::{
    evaluation::{EvalLimits, NoBindings},
    functions::{conversions::text::NumberFormat, text::cstr::DateTextFormat},
};
use std::cell::Cell;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{Currency, Decimal, Double, Guid, LegacyDateTime},
};
use tablecraft_sql::{
    parse_select,
    select::{Expression, Projection},
};

fn expression(sql: &str) -> Expression {
    let parsed = parse_select(&format!("SELECT {sql}"), Default::default()).unwrap();
    let Projection::Expression { expression, .. } = parsed.query().projection[0].clone() else {
        panic!("expression required");
    };
    expression
}

#[test]
fn concatenation_formats_supported_scalars_without_changing_plus_or_null_rules() {
    let mut budget = ResourceBudget::new(100_000, 0, 100_000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, Default::default())
        .with_number_format(NumberFormat::DeDe)
        .with_date_format(DateTextFormat::Iso);
    for (sql, expected) in [
        (
            "'ID=' & 9007199254740993",
            Value::Text("ID=9007199254740993".into()),
        ),
        (
            "CCur(1.25) & ':' & CDec(2.5)",
            Value::Text("1,25:2,5".into()),
        ),
        ("True & False", Value::Text("TrueFalse".into())),
        ("#2024-01-02# & 'x'", Value::Text("2024-01-02x".into())),
        ("Null & 2", Value::Text("2".into())),
        ("2 & Null", Value::Text("2".into())),
        ("Null & Null", Value::Null),
        ("'2'+'3'", Value::Text("23".into())),
        ("'2'+Null", Value::Null),
        ("'2'+3", Value::Double(Double::from_float(5.0))),
    ] {
        assert_eq!(evaluator.evaluate(&expression(sql)), Ok(expected), "{sql}");
    }
}

#[test]
fn missing_policies_and_eager_failures_remain_distinct() {
    for (sql, error) in [
        ("2 & 'x'", EvalError::NumberFormatRequired),
        ("Null & 2", EvalError::NumberFormatRequired),
        ("#2024-01-02# & 'x'", EvalError::DateFormatRequired),
        ("2 & (1/0)", EvalError::DivisionByZero),
        ("Null & (1/0)", EvalError::DivisionByZero),
        ("CVErr(1) & 'x'", EvalError::NonScalarExpression),
    ] {
        let mut budget = ResourceBudget::new(100_000, 0, 100_000);
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, Default::default()).evaluate(&expression(sql)),
            Err(error),
            "{sql}"
        );
    }
    let mut budget = ResourceBudget::new(100, 0, 100);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, Default::default())
            .evaluate(&expression("True & 'x'")),
        Ok(Value::Text("Truex".into()))
    );
}

#[test]
fn conversions_and_final_join_charge_cumulative_bytes_before_allocation() {
    for (bytes, accepted) in [(3, false), (4, true)] {
        let mut budget = ResourceBudget::new(bytes, 0, 100);
        let result = Evaluator::new(&NoBindings, &mut budget, Default::default())
            .with_number_format(NumberFormat::EnUs)
            .concatenate(&Value::Int32(12), &Value::Null);
        if accepted {
            assert_eq!(result, Ok(Value::Text("12".into())));
            assert_eq!(budget.remaining(Resource::Bytes), 0);
        } else {
            assert!(
                matches!(result, Err(EvalError::Limit(error)) if error.resource == Resource::Bytes)
            );
        }
    }
    let mut budget = ResourceBudget::new(100, 0, 100);
    let result = Evaluator::new(
        &NoBindings,
        &mut budget,
        EvalLimits {
            max_text_bytes: 3,
            ..Default::default()
        },
    )
    .with_number_format(NumberFormat::EnUs)
    .concatenate(&Value::Int32(12), &Value::Int32(34));
    assert_eq!(result, Err(EvalError::TextLimit));
}

#[test]
fn each_observed_conversion_checkpoint_can_cancel_without_mutating_inputs() {
    let left = Value::Currency(Currency::from_scaled(-12345));
    let right = Value::Decimal(Decimal::new(98765, 3).unwrap());
    let calls = Cell::new(0);
    let run = |stop| {
        calls.set(0);
        let cancelled = || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        };
        let mut budget = ResourceBudget::new(100_000, 0, 100_000);
        Evaluator::new(&NoBindings, &mut budget, Default::default())
            .with_number_format(NumberFormat::EnUs)
            .with_cancellation(&cancelled)
            .concatenate(&left, &right)
    };
    assert_eq!(run(usize::MAX), Ok(Value::Text("-1.234598.765".into())));
    let count = calls.get();
    assert!(count > 0);
    for stop in 1..=count {
        assert_eq!(run(stop), Err(EvalError::Cancelled), "checkpoint {stop}");
    }
    assert_eq!(left, Value::Currency(Currency::from_scaled(-12345)));
    assert_eq!(right, Value::Decimal(Decimal::new(98765, 3).unwrap()));
}

#[test]
fn implicit_binary_and_guid_rendering_remain_refused() {
    let mut budget = ResourceBudget::new(1000, 0, 1000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, Default::default());
    assert_eq!(
        evaluator.concatenate(&Value::Binary(vec![1]), &Value::Null),
        Err(EvalError::TypeMismatch)
    );
    assert_eq!(
        evaluator.concatenate(&Value::Guid(Guid::from_bytes([0; 16])), &Value::Null),
        Err(EvalError::TypeMismatch)
    );
    let date = Value::DateTime(LegacyDateTime::from_ole_days(2.0).unwrap());
    assert_eq!(
        evaluator.concatenate(&date, &Value::Null),
        Err(EvalError::DateFormatRequired)
    );
}

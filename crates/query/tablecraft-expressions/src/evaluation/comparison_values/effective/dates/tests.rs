use super::*;
use crate::evaluation::{EvalLimits, NoBindings};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Currency, Decimal, Double, ExtendedDateTime, LegacyDateTime, Single},
};

fn date(serial: f64) -> Value {
    Value::DateTime(LegacyDateTime::from_ole_days(serial).unwrap())
}

#[test]
fn date_selected_numeric_comparison_uses_calendar_order_and_checked_envelopes() {
    let mut budget = ResourceBudget::new(65536, 0, 65536);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    for number in [
        Value::Byte(1),
        Value::Int16(1),
        Value::Int32(1),
        Value::Int64(1),
        Value::Single(Single::from_float(1.0)),
        Value::Double(Double::from_float(1.0)),
        Value::Currency(Currency::from_scaled(10000)),
    ] {
        assert_eq!(
            evaluator.compare_effective_numeric_values(Comparison::Equal, &date(1.0), &number),
            Ok(Truth::True)
        );
        assert_eq!(
            evaluator.compare_effective_numeric_values(Comparison::Equal, &number, &date(1.0)),
            Ok(Truth::True)
        );
    }
    assert_eq!(
        evaluator.compare_effective_numeric_values(
            Comparison::Equal,
            &date(-1.0),
            &Value::Boolean(true)
        ),
        Ok(Truth::True)
    );
    assert_eq!(
        evaluator.compare_effective_numeric_values(
            Comparison::Equal,
            &date(-0.25),
            &Value::Double(Double::from_float(0.25))
        ),
        Ok(Truth::True)
    );
    assert_eq!(
        evaluator.compare_effective_numeric_values(
            Comparison::Less,
            &date(-1.25),
            &Value::Double(Double::from_float(-1.75))
        ),
        Ok(Truth::True)
    );
    for number in [Value::Int32(2958466), Value::Int32(-657435)] {
        assert_eq!(
            evaluator.compare_effective_numeric_values(Comparison::Equal, &date(1.0), &number),
            Err(EvalError::Overflow)
        );
    }
    assert_eq!(
        evaluator.compare_effective_numeric_values(
            Comparison::Equal,
            &date(1.0),
            &Value::Double(Double::from_float(f64::INFINITY))
        ),
        Err(EvalError::NonFinite)
    );
    assert_eq!(
        evaluator.compare_values(Comparison::Equal, &date(1.0), &Value::Int16(1)),
        Ok(Truth::True)
    );
}

#[test]
fn decimal_date_comparisons_keep_raw_serial_selection() {
    let mut budget = ResourceBudget::new(65536, 0, 65536);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    let negative = Value::Decimal(Decimal::new(-25, 2).unwrap());
    let positive = Value::Decimal(Decimal::new(25, 2).unwrap());
    assert_eq!(
        evaluator.compare_effective_numeric_values(Comparison::Equal, &date(-0.25), &negative),
        Ok(Truth::True)
    );
    assert_eq!(
        evaluator.compare_effective_numeric_values(Comparison::Equal, &negative, &date(-0.25)),
        Ok(Truth::True)
    );
    assert_eq!(
        evaluator.compare_effective_numeric_values(Comparison::Equal, &date(-0.25), &positive),
        Ok(Truth::False)
    );
    // Decimal selection does not constrain its other operand to the Date envelope.
    let large = Value::Decimal(Decimal::new(2958466, 0).unwrap());
    assert_eq!(
        evaluator.compare_effective_numeric_values(Comparison::Less, &date(1.0), &large),
        Ok(Truth::True)
    );
}

#[test]
fn date_policy_null_extended_precision_and_cancellation_boundaries_remain_explicit() {
    let mut budget = ResourceBudget::new(65536, 0, 65536);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    assert_eq!(
        evaluator.compare_effective_numeric_values(
            Comparison::Equal,
            &date(0.0),
            &Value::Text("1899-12-30".into())
        ),
        Err(EvalError::DateFormatRequired)
    );
    assert_eq!(
        evaluator.compare_effective_numeric_values(Comparison::Equal, &date(0.0), &Value::Null),
        Ok(Truth::Unknown)
    );
    let extended = Value::DateTimeExtended(ExtendedDateTime::new(2030, 1, 1, 0, 0, 0, 1).unwrap());
    assert_eq!(
        evaluator.compare_effective_numeric_values(Comparison::Equal, &date(0.0), &extended),
        Err(EvalError::TypeMismatch)
    );
    let mut cancelled_budget = ResourceBudget::new(0, 0, 1);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut cancelled_budget, EvalLimits::default())
            .with_cancellation(&|| true)
            .compare_effective_numeric_values(Comparison::Equal, &date(0.0), &Value::Byte(0)),
        Err(EvalError::Cancelled)
    );
}

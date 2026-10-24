use super::*;
use crate::evaluation::{EvalLimits, NoBindings};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Decimal, Double, LegacyDateTime},
};

fn date(serial: f64) -> Value {
    Value::DateTime(LegacyDateTime::from_ole_days(serial).unwrap())
}

fn truth(value: bool) -> Truth {
    match value {
        true => Truth::True,
        false => Truth::False,
    }
}

fn calendar_quarters(quarters: i32) -> i32 {
    if quarters < 0 {
        (quarters / 4) * 4 - quarters % 4
    } else {
        quarters
    }
}

#[test]
fn ordinary_predicates_use_calendar_order_unless_decimal_selects_serial_order() {
    let mut budget = ResourceBudget::new(1_048_576, 0, 1_048_576);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    for left in -8..=8 {
        for right in -8..=8 {
            let moment = date(f64::from(left) / 4.0);
            let number = Value::Double(Double::from_float(f64::from(right) / 4.0));
            let decimal = Value::Decimal(Decimal::new(i128::from(right) * 25, 2).unwrap());
            let a = calendar_quarters(left);
            let b = calendar_quarters(right);
            for (operator, temporal, serial) in [
                (Comparison::Equal, a == b, left == right),
                (Comparison::NotEqual, a != b, left != right),
                (Comparison::Less, a < b, left < right),
                (Comparison::LessOrEqual, a <= b, left <= right),
                (Comparison::Greater, a > b, left > right),
                (Comparison::GreaterOrEqual, a >= b, left >= right),
            ] {
                assert_eq!(
                    evaluator.compare_values(operator, &moment, &number),
                    Ok(truth(temporal))
                );
                assert_eq!(
                    evaluator.compare_values(operator, &moment, &decimal),
                    Ok(truth(serial))
                );
            }
        }
    }
}

#[test]
fn ordinary_date_conversion_checks_ranges_without_changing_strict_keys() {
    let mut budget = ResourceBudget::new(65536, 0, 65536);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    for value in [Value::Int32(-657435), Value::Int32(2958466)] {
        for (left, right) in [(date(1.0), value.clone()), (value.clone(), date(1.0))] {
            assert_eq!(
                evaluator.compare_values(Comparison::Less, &left, &right),
                Err(EvalError::Overflow)
            );
        }
    }
    assert_eq!(
        evaluator.compare_values(Comparison::Equal, &date(1.0), &Value::Null),
        Ok(Truth::Unknown)
    );
    assert_eq!(
        comparison::compare(Comparison::Equal, &date(1.0), &Value::Int16(1), None),
        Err(EvalError::TypeMismatch)
    );
}

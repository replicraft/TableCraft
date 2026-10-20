//! Automation DATE serial operators, separate from Gregorian interval functions.
use crate::{EvalError, functions::conversions::Conversion, numeric::Arithmetic};
use tablecraft_core::values::{Double, LegacyDateTime, Value};

/// Add/subtract stored Automation DATE serials and numeric day operands.
/// At least one non-NULL operand must be legacy DateTime. Numeric operands use
/// the explicit CDbl policy; dates use their exact stored serial, not a rounded
/// calendar projection. Date-minus-Date returns Double. Other results retain Date
/// when representable, promoting finite out-of-envelope serials to Double.
/// NULL propagates; text, extended DateTime and other operators are unsupported.
/// This serial policy does not claim elapsed-time semantics before the epoch.
pub fn date_arithmetic(
    operator: Arithmetic,
    left: &Value,
    right: &Value,
) -> Result<Value, EvalError> {
    if !matches!(operator, Arithmetic::Add | Arithmetic::Subtract) {
        return Err(EvalError::UnsupportedExpression);
    }
    if matches!(left, Value::Null) || matches!(right, Value::Null) {
        return Ok(Value::Null);
    }
    let left_date = matches!(left, Value::DateTime(_));
    let right_date = matches!(right, Value::DateTime(_));
    if !left_date && !right_date {
        return Err(EvalError::TypeMismatch);
    }
    let a = serial(left)?;
    let b = serial(right)?;
    let result = if operator == Arithmetic::Add {
        a + b
    } else {
        a - b
    };
    if !result.is_finite() {
        return Err(EvalError::Overflow);
    }
    if operator == Arithmetic::Subtract && left_date && right_date {
        return Ok(Value::Double(Double::from_float(result)));
    }
    Ok(match LegacyDateTime::from_ole_days(result) {
        Ok(value) => Value::DateTime(value),
        Err(_) => Value::Double(Double::from_float(result)),
    })
}
fn serial(value: &Value) -> Result<f64, EvalError> {
    if let Value::DateTime(value) = value {
        return Ok(value.ole_days());
    }
    let Value::Double(value) = Conversion::Double.apply(value)? else {
        return Err(EvalError::TypeMismatch);
    };
    Ok(value.to_float())
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use tablecraft_core::values::{Currency, Decimal, Single};
    fn date(serial: f64) -> Value {
        Value::DateTime(LegacyDateTime::from_ole_days(serial).unwrap())
    }
    fn double(value: f64) -> Value {
        Value::Double(Double::from_float(value))
    }
    #[test]
    fn operand_order_and_result_carriers_follow_explicit_serial_arithmetic() {
        for number in [
            Value::Byte(1),
            Value::Int16(1),
            Value::Int32(1),
            Value::Int64(1),
            Value::Single(Single::from_float(1.0)),
            double(1.0),
            Value::Currency(Currency::from_scaled(10000)),
            Value::Decimal(Decimal::new(100, 2).unwrap()),
        ] {
            assert_eq!(
                date_arithmetic(Arithmetic::Add, &date(2.25), &number),
                Ok(date(3.25))
            );
            assert_eq!(
                date_arithmetic(Arithmetic::Add, &number, &date(2.25)),
                Ok(date(3.25))
            );
            assert_eq!(
                date_arithmetic(Arithmetic::Subtract, &date(2.25), &number),
                Ok(date(1.25))
            );
            assert_eq!(
                date_arithmetic(Arithmetic::Subtract, &number, &date(2.25)),
                Ok(date(-1.25))
            );
        }
        assert_eq!(
            date_arithmetic(Arithmetic::Add, &date(1.25), &date(2.25)),
            Ok(date(3.5))
        );
        assert_eq!(
            date_arithmetic(Arithmetic::Subtract, &date(1.25), &date(2.25)),
            Ok(double(-1.0))
        );
        assert_eq!(
            date_arithmetic(Arithmetic::Add, &date(2.25), &Value::Boolean(true)),
            Ok(date(1.25))
        );
        // Equal calendar values have distinct stored DATE serials; these operators
        // deliberately do not silently normalize either representation.
        assert_eq!(
            date_arithmetic(Arithmetic::Subtract, &date(-0.25), &date(0.25)),
            Ok(double(-0.5))
        );
        assert_eq!(
            date_arithmetic(Arithmetic::Add, &date(-1.25), &double(0.5)),
            Ok(date(-0.75))
        );
    }
    #[test]
    fn finite_date_overflow_promotes_and_invalid_operands_remain_errors() {
        assert_eq!(
            date_arithmetic(Arithmetic::Add, &date(2958465.0), &Value::Int32(1)),
            Ok(double(2958466.0))
        );
        assert_eq!(
            date_arithmetic(Arithmetic::Subtract, &date(-657434.0), &Value::Int32(1)),
            Ok(double(-657435.0))
        );
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(
                date_arithmetic(Arithmetic::Add, &date(1.0), &double(value)),
                Err(EvalError::NonFinite)
            );
        }
        assert_eq!(
            date_arithmetic(Arithmetic::Add, &date(1.0), &Value::Text("1".into())),
            Err(EvalError::TypeMismatch)
        );
        assert_eq!(
            date_arithmetic(Arithmetic::Add, &date(1.0), &Value::Null),
            Ok(Value::Null)
        );
        assert_eq!(
            date_arithmetic(Arithmetic::Multiply, &date(1.0), &Value::Int32(2)),
            Err(EvalError::UnsupportedExpression)
        );
        assert_eq!(
            crate::numeric::arithmetic(Arithmetic::Add, &date(1.0), &Value::Int32(2)),
            Err(EvalError::TypeMismatch)
        );
    }
    proptest! {
        #![proptest_config(proptest::test_runner::Config { failure_persistence: None, ..Default::default() })]
        #[test]
        fn quarter_day_serial_shifts_match_an_independent_integer_grid(day in -100000_i32..100000_i32, quarters in -10000_i32..10000_i32) {
            let serial=f64::from(day);let offset=f64::from(quarters)/4.0;
            let expected=f64::from(day*4+quarters)/4.0;
            prop_assert_eq!(date_arithmetic(Arithmetic::Add,&date(serial),&double(offset)),Ok(date(expected)));
            prop_assert_eq!(date_arithmetic(Arithmetic::Subtract,&date(expected),&date(serial)),Ok(double(offset)));
        }
    }
}

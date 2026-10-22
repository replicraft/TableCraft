use super::*;
use tablecraft_core::values::{Currency, Decimal, LegacyDateTime};

#[test]
fn unary_minus_preserves_width_widens_small_minima_and_checks_large_minima() {
    for (input, expected) in [
        (Value::Byte(0), Value::Int16(0)),
        (Value::Byte(255), Value::Int16(-255)),
        (Value::Boolean(true), Value::Int16(1)),
        (Value::Int16(1), Value::Int16(-1)),
        (Value::Int16(i16::MIN), Value::Int32(32768)),
        (
            Value::Int32(i32::MIN),
            Value::Double(Double::from_float(2147483648.0)),
        ),
        (
            Value::Single(Single::from_float(0.0)),
            Value::Single(Single::from_float(-0.0)),
        ),
        (
            Value::Currency(Currency::from_scaled(1)),
            Value::Currency(Currency::from_scaled(-1)),
        ),
        (
            Value::Decimal(Decimal::new(25, 1).unwrap()),
            Value::Decimal(Decimal::new(-25, 1).unwrap()),
        ),
        (Value::Null, Value::Null),
    ] {
        assert_eq!(promoted_negate(&input), Ok(expected));
    }
    assert_eq!(
        promoted_negate(&Value::Int64(i64::MIN)),
        Err(EvalError::Overflow)
    );
    assert_eq!(
        promoted_negate(&Value::Currency(Currency::from_scaled(i64::MIN))),
        Err(EvalError::Overflow)
    );
    assert_eq!(
        promoted_negate(&Value::Single(Single::from_float(f32::NAN))),
        Err(EvalError::NonFinite)
    );
}

#[test]
fn division_power_and_rounded_operators_select_their_own_carriers() {
    let single = |value| Value::Single(Single::from_float(value));
    let double = |value| Value::Double(Double::from_float(value));
    let currency = |value| Value::Currency(Currency::from_scaled(value));
    for (operator, left, right, expected) in [
        (
            Arithmetic::Divide,
            single(3.0),
            Value::Int16(2),
            single(1.5),
        ),
        (
            Arithmetic::Divide,
            single(3.0),
            Value::Int32(2),
            double(1.5),
        ),
        (
            Arithmetic::Divide,
            currency(30000),
            currency(20000),
            double(1.5),
        ),
        (
            Arithmetic::Divide,
            Value::DateTime(LegacyDateTime::from_ole_days(-1.25).unwrap()),
            Value::Int16(2),
            double(-0.625),
        ),
        (
            Arithmetic::Power,
            currency(30000),
            Value::Byte(2),
            double(9.0),
        ),
        (
            Arithmetic::IntegerDivide,
            Value::Byte(9),
            Value::Byte(2),
            Value::Byte(4),
        ),
        (
            Arithmetic::IntegerDivide,
            Value::Int16(-9),
            Value::Int16(2),
            Value::Int16(-4),
        ),
        (
            Arithmetic::IntegerDivide,
            Value::Int16(i16::MIN),
            Value::Int16(-1),
            Value::Int32(32768),
        ),
        (
            Arithmetic::IntegerDivide,
            Value::Int32(i32::MIN),
            Value::Int32(-1),
            double(2147483648.0),
        ),
        (
            Arithmetic::IntegerDivide,
            Value::Int16(9),
            double(2.5),
            Value::Int16(4),
        ),
        (
            Arithmetic::IntegerDivide,
            double(9.5),
            Value::Int16(2),
            Value::Int32(5),
        ),
        (
            Arithmetic::Modulo,
            double(-9.5),
            Value::Int16(3),
            Value::Int32(-1),
        ),
    ] {
        assert_eq!(promoted_arithmetic(operator, &left, &right), Ok(expected));
    }
    assert_eq!(
        promoted_arithmetic(
            Arithmetic::IntegerDivide,
            &Value::Int16(9),
            &double(32767.5)
        ),
        Err(EvalError::Overflow)
    );
    assert_eq!(
        promoted_arithmetic(
            Arithmetic::IntegerDivide,
            &Value::Int64(i64::MIN),
            &Value::Int16(-1)
        ),
        Err(EvalError::Overflow)
    );
    assert_eq!(
        promoted_arithmetic(
            Arithmetic::Modulo,
            &Value::Int64(i64::MIN),
            &Value::Int16(-1)
        ),
        Ok(Value::Int64(0))
    );
    assert_eq!(
        promoted_arithmetic(Arithmetic::Divide, &single(f32::MAX), &single(0.5)),
        Ok(double(f64::from(f32::MAX) * 2.0))
    );
}

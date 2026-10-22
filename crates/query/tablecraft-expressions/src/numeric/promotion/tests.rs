use super::*;
use proptest::prelude::*;
use tablecraft_core::values::{Currency, Decimal, LegacyDateTime};

#[test]
fn integral_widths_widen_only_after_their_effective_range_is_exceeded() {
    for (left, right, expected) in [
        (Value::Byte(1), Value::Byte(2), Value::Byte(3)),
        (Value::Byte(255), Value::Byte(1), Value::Int16(256)),
        (Value::Boolean(true), Value::Byte(1), Value::Int16(0)),
        (Value::Int16(32767), Value::Int16(1), Value::Int32(32768)),
        (Value::Int32(1), Value::Int16(1), Value::Int32(2)),
        (
            Value::Int32(i32::MAX),
            Value::Int16(1),
            Value::Double(Double::from_float(2147483648.0)),
        ),
        (Value::Int64(1), Value::Byte(1), Value::Int64(2)),
    ] {
        assert_eq!(
            promoted_arithmetic(Arithmetic::Add, &left, &right),
            Ok(expected)
        );
    }
    assert_eq!(
        promoted_arithmetic(Arithmetic::Subtract, &Value::Byte(0), &Value::Byte(1)),
        Ok(Value::Int16(-1))
    );
    assert_eq!(
        promoted_arithmetic(
            Arithmetic::Multiply,
            &Value::Int16(32767),
            &Value::Int16(32767)
        ),
        Ok(Value::Int32(1073676289))
    );
    assert_eq!(
        promoted_arithmetic(Arithmetic::Add, &Value::Int64(i64::MAX), &Value::Byte(1)),
        Err(EvalError::Overflow)
    );
}

#[test]
fn single_results_keep_single_until_overflow_and_long_participation_selects_double() {
    let single = |value| Value::Single(Single::from_float(value));
    let double = |value| Value::Double(Double::from_float(value));
    assert_eq!(
        promoted_arithmetic(Arithmetic::Add, &single(1.25), &Value::Int16(2)),
        Ok(single(3.25))
    );
    assert_eq!(
        promoted_arithmetic(Arithmetic::Add, &single(1.25), &Value::Int32(2)),
        Ok(double(3.25))
    );
    assert_eq!(
        promoted_arithmetic(Arithmetic::Multiply, &single(f32::MAX), &single(2.0)),
        Ok(double(f64::from(f32::MAX) * 2.0))
    );
    assert_eq!(
        promoted_arithmetic(
            Arithmetic::Multiply,
            &single(f32::MIN_POSITIVE),
            &single(f32::MIN_POSITIVE)
        ),
        Ok(single(0.0))
    );
    assert_eq!(
        promoted_arithmetic(
            Arithmetic::Add,
            &single(1.0),
            &Value::Int64(9007199254740993)
        ),
        Ok(double(9007199254740992.0))
    );
}

#[test]
fn fixed_point_and_date_selection_reuse_checked_library_conversions() {
    let currency = |value| Value::Currency(Currency::from_scaled(value));
    let decimal = |coefficient, scale| Value::Decimal(Decimal::new(coefficient, scale).unwrap());
    let date = Value::DateTime(LegacyDateTime::from_ole_days(2.25).unwrap());
    assert_eq!(
        promoted_arithmetic(Arithmetic::Multiply, &currency(15000), &currency(15000)),
        Ok(currency(22500))
    );
    assert_eq!(
        promoted_arithmetic(Arithmetic::Multiply, &currency(1), &currency(5000)),
        Ok(currency(0))
    );
    assert_eq!(
        promoted_arithmetic(Arithmetic::Multiply, &currency(3), &currency(5000)),
        Ok(currency(2))
    );
    assert_eq!(
        promoted_arithmetic(
            Arithmetic::Add,
            &currency(1),
            &Value::Double(Double::from_float(0.00005))
        ),
        // The exact binary64 value of 0.00005 lies above the decimal midpoint.
        Ok(currency(2))
    );
    assert_eq!(
        promoted_arithmetic(
            Arithmetic::Multiply,
            &currency(15000),
            &Value::Double(Double::from_float(2.0))
        ),
        Ok(Value::Double(Double::from_float(3.0)))
    );
    assert_eq!(
        promoted_arithmetic(Arithmetic::Add, &date, &decimal(1, 0)),
        Ok(decimal(325, 2))
    );
    assert_eq!(
        promoted_arithmetic(Arithmetic::Multiply, &date, &Value::Int16(2)),
        Ok(Value::Double(Double::from_float(4.5)))
    );
    assert_eq!(
        promoted_arithmetic(Arithmetic::Multiply, &currency(i64::MAX), &currency(20000)),
        Err(EvalError::Overflow)
    );
    assert_eq!(
        promoted_arithmetic(Arithmetic::Add, &Value::Text("1".into()), &Value::Int16(1)),
        Err(EvalError::TypeMismatch)
    );
    assert_eq!(
        promoted_arithmetic(Arithmetic::Add, &Value::Null, &Value::Int16(1)),
        Ok(Value::Null)
    );
}

proptest! {
    #![proptest_config(crate::test_support::config())]
    #[test]
    fn integer_short_operands_match_an_independent_wide_model(a in any::<i16>(), b in any::<i16>()) {
        for (operator, expected) in [(Arithmetic::Add, i64::from(a)+i64::from(b)), (Arithmetic::Subtract, i64::from(a)-i64::from(b)), (Arithmetic::Multiply, i64::from(a)*i64::from(b))] {
            let expected = match i16::try_from(expected) {
                Ok(value) => Value::Int16(value),
                Err(_) => Value::Int32(i32::try_from(expected).unwrap()),
            };
            prop_assert_eq!(promoted_arithmetic(operator, &Value::Int16(a), &Value::Int16(b)), Ok(expected));
        }
    }
}

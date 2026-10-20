use super::super::Conversion;
use super::*;
use proptest::prelude::*;
use tablecraft_core::values::{Currency, Decimal, Double, Single};

fn decimal(coefficient: i128, scale: u8) -> Value {
    Value::Decimal(Decimal::new(coefficient, scale).unwrap())
}
#[test]
fn exact_conversions_preserve_large_coefficients_scale_and_boolean_sign() {
    for (input, expected) in [
        (Value::Boolean(true), decimal(-1, 0)),
        (Value::Boolean(false), decimal(0, 0)),
        (Value::Byte(255), decimal(255, 0)),
        (Value::Int16(i16::MIN), decimal(i128::from(i16::MIN), 0)),
        (Value::Int32(i32::MAX), decimal(i128::from(i32::MAX), 0)),
        (Value::Int64(i64::MIN), decimal(i128::from(i64::MIN), 0)),
        (Value::Int64(9007199254740993), decimal(9007199254740993, 0)),
        (
            Value::Currency(Currency::from_scaled(i64::MIN)),
            decimal(i128::from(i64::MIN), 4),
        ),
        (
            decimal(Decimal::MAX_COEFFICIENT, 28),
            decimal(Decimal::MAX_COEFFICIENT, 28),
        ),
        (
            decimal(-Decimal::MAX_COEFFICIENT, 0),
            decimal(-Decimal::MAX_COEFFICIENT, 0),
        ),
    ] {
        assert_eq!(Conversion::Decimal.apply(&input), Ok(expected));
    }
}
#[test]
fn normalized_float_profile_is_bounded_and_distinct_from_currency_retention() {
    for (input, expected) in [
        (Value::Double(Double::from_float(0.1)), decimal(1, 1)),
        (Value::Single(Single::from_float(0.1)), decimal(1, 1)),
        (Value::Double(Double::from_float(1.25)), decimal(125, 2)),
        (
            Value::Double(Double::from_float(2_f64.powi(95))),
            decimal(39614081257132168796771975168, 0),
        ),
        (
            Value::Double(Double::from_float(f64::from_bits(1))),
            decimal(0, 0),
        ),
        (Value::Double(Double::from_float(-0.0)), decimal(0, 0)),
    ] {
        assert_eq!(Conversion::Decimal.apply(&input), Ok(expected));
    }
    for input in [
        Value::Double(Double::from_float(2_f64.powi(96))),
        Value::Double(Double::from_float(f64::MAX)),
    ] {
        assert_eq!(Conversion::Decimal.apply(&input), Err(EvalError::Overflow));
    }
    assert_eq!(
        Conversion::Decimal.apply(&Value::Double(Double::from_float(f64::NAN))),
        Err(EvalError::NonFinite)
    );
    assert_eq!(
        Conversion::Decimal.apply(&Value::Text("0.1".into())),
        Err(EvalError::TypeMismatch)
    );
    assert_eq!(
        Conversion::Decimal.apply(&Value::Null),
        Err(EvalError::InvalidNullConversion)
    );
}
proptest! {
    #![proptest_config(crate::test_support::config())]
    #[test]
    fn every_decimal_representation_is_an_identity(coefficient in -Decimal::MAX_COEFFICIENT..=Decimal::MAX_COEFFICIENT,scale in 0_u8..=28) {
        let input=decimal(coefficient,scale);
        prop_assert_eq!(Conversion::Decimal.apply(&input),Ok(input));
    }
    #[test]
    fn integer_and_currency_conversions_are_exact_without_float_intermediates(integer in any::<i64>()) {
        prop_assert_eq!(Conversion::Decimal.apply(&Value::Int64(integer)),Ok(decimal(i128::from(integer),0)));
        prop_assert_eq!(Conversion::Decimal.apply(&Value::Currency(Currency::from_scaled(integer))),Ok(decimal(i128::from(integer),4)));
    }
}

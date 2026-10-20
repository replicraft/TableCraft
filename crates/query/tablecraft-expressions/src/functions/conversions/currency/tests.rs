use super::super::Conversion;
use super::*;
use proptest::prelude::*;
use tablecraft_core::values::{Decimal as FieldDecimal, Double, Single};
fn expected(scaled: i64) -> Result<Value, EvalError> {
    Ok(Value::Currency(Currency::from_scaled(scaled)))
}
#[test]
fn exact_rounding_bounds_boolean_null_and_identity_are_checked() {
    for (coefficient, scale, scaled) in [
        (123445, 5, 12344),
        (123455, 5, 12346),
        (-123445, 5, -12344),
        (-123455, 5, -12346),
        (9223372036854775807, 4, i64::MAX),
        (-9223372036854775808, 4, i64::MIN),
        (92233720368547758065, 5, i64::MAX - 1),
        (-92233720368547758085, 5, i64::MIN),
    ] {
        assert_eq!(
            Conversion::Currency.apply(&Value::Decimal(
                FieldDecimal::new(coefficient, scale).unwrap()
            )),
            expected(scaled)
        );
    }
    assert_eq!(
        Conversion::Currency.apply(&Value::Decimal(
            FieldDecimal::new(92233720368547758075, 5).unwrap()
        )),
        Err(EvalError::Overflow)
    );
    assert_eq!(
        Conversion::Currency.apply(&Value::Int64(i64::MAX)),
        Err(EvalError::Overflow)
    );
    assert_eq!(
        Conversion::Currency.apply(&Value::Boolean(true)),
        expected(-10000)
    );
    assert_eq!(
        Conversion::Currency.apply(&Value::Boolean(false)),
        expected(0)
    );
    assert_eq!(
        Conversion::Currency.apply(&Value::Null),
        Err(EvalError::InvalidNullConversion)
    );
    assert_eq!(
        Conversion::Currency.apply(&Value::Text("1.25".into())),
        Err(EvalError::TypeMismatch)
    );
    assert_eq!(
        Conversion::Currency.apply(&Value::Double(Double::from_float(f64::NAN))),
        Err(EvalError::NonFinite)
    );
    assert_eq!(
        Conversion::Currency.apply(&Value::Double(Double::from_float(f64::MAX))),
        Err(EvalError::Overflow)
    );
}
#[test]
fn retained_ieee_inputs_do_not_use_their_shortest_decimal_spelling() {
    assert_eq!(
        Conversion::Currency.apply(&Value::Double(Double::from_float(0.00005))),
        expected(1)
    );
    assert_eq!(
        Conversion::Currency.apply(&Value::Double(Double::from_float(0.03125))),
        expected(312)
    );
    assert_eq!(
        Conversion::Currency.apply(&Value::Double(Double::from_float(f64::from_bits(
            0.03125_f64.to_bits() + 1
        )))),
        expected(313)
    );
    for value in [0.0, -0.0, f64::MIN_POSITIVE, f64::from_bits(1)] {
        assert_eq!(
            Conversion::Currency.apply(&Value::Double(Double::from_float(value))),
            expected(0)
        );
    }
    assert_eq!(
        Conversion::Currency.apply(&Value::Single(Single::from_float(1.25))),
        expected(12500)
    );
}
proptest! {
    #![proptest_config(crate::test_support::config())]
    #[test]
    fn every_currency_payload_is_an_identity(scaled in any::<i64>()) {
        let original=Value::Currency(Currency::from_scaled(scaled));
        prop_assert_eq!(Conversion::Currency.apply(&original),Ok(original));
    }
}

fn exact_ieee_currency(value: f64) -> Result<Value, EvalError> {
    // Independent exact binary-rational oracle for the generated exponent range.
    if value.abs() < 0.00001 {
        return expected(0);
    }
    let bits = value.to_bits();
    let fraction_bits = f64::MANTISSA_DIGITS - 1;
    let mantissa = (bits & ((1_u64 << fraction_bits) - 1)) | (1_u64 << fraction_bits);
    let power = ((bits >> fraction_bits) & 0x7ff) as i32 - 1023 - fraction_bits as i32;
    let mut numerator = i128::from(mantissa) * 10000;
    let denominator = if power < 0 {
        1_i128 << (-power)
    } else {
        numerator <<= power;
        1
    };
    let whole = numerator / denominator;
    let remainder = numerator % denominator;
    let rounded = whole
        + i128::from(
            remainder * 2 > denominator || (remainder * 2 == denominator && whole % 2 != 0),
        );
    let signed = if value.is_sign_negative() {
        -rounded
    } else {
        rounded
    };
    signed
        .try_into()
        .map(Currency::from_scaled)
        .map(Value::Currency)
        .map_err(|_| EvalError::Overflow)
}
proptest! {
    #![proptest_config(crate::test_support::config())]
    #[test]
    fn retained_float_rounding_agrees_with_exact_binary_rationals(mantissa in 0_u64..(1_u64<<52), exponent in -20_i32..=49, negative in any::<bool>()) {
        let bits=((exponent+1023) as u64)<<52 | mantissa | if negative {1_u64<<63} else {0};
        let value=f64::from_bits(bits);
        prop_assert_eq!(Conversion::Currency.apply(&Value::Double(Double::from_float(value))),exact_ieee_currency(value));
    }
}

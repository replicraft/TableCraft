use super::super::Conversion;
use super::*;
use proptest::prelude::*;
use tablecraft_core::values::Currency;

#[test]
fn decimal_rounding_targets_single_directly_and_currency_does_not_round_twice() {
    // One 10^-27 above the binary32 midpoint between 1 and its successor.
    let decimal = Value::Decimal(Decimal::new(1000000059604644775390625001, 27).unwrap());
    assert_eq!(
        Conversion::Single.apply(&decimal),
        Ok(Value::Single(Single::from_bits(0x3f800001)))
    );
    let decimal = Value::Decimal(Decimal::new(1000000059604644775390625000, 27).unwrap());
    assert_eq!(
        Conversion::Single.apply(&decimal),
        Ok(Value::Single(Single::from_bits(0x3f800000)))
    );
    let decimal = Value::Decimal(Decimal::new(-1000000059604644775390625001, 27).unwrap());
    assert_eq!(
        Conversion::Single.apply(&decimal),
        Ok(Value::Single(Single::from_bits(0xbf800001)))
    );
    // Independent exact-rational/IEEE golden, not coefficient-as-f64 / 10^4.
    let currency = Value::Currency(Currency::from_scaled(6811109015807608224));
    assert_eq!(
        Conversion::Double.apply(&currency),
        Ok(Value::Double(Double::from_bits(0x43035bbbeabb1ec7)))
    );
    for scale in 0..=Decimal::MAX_SCALE {
        let value = Decimal::new(-Decimal::MAX_COEFFICIENT, scale).unwrap();
        assert!(literal(&value).unwrap().as_str().is_ok());
        assert!(Conversion::Single.apply(&Value::Decimal(value)).is_ok());
        assert!(Conversion::Double.apply(&Value::Decimal(value)).is_ok());
    }
    assert!(
        literal(&Currency::from_scaled(i64::MIN))
            .unwrap()
            .as_str()
            .is_ok()
    );
}
#[test]
fn finite_range_signed_zero_subnormal_null_and_explicit_rounding_are_distinct() {
    for conversion in [Conversion::Single, Conversion::Double] {
        assert_eq!(
            conversion.apply(&Value::Null),
            Err(EvalError::InvalidNullConversion)
        );
        assert_eq!(
            conversion.apply(&Value::Text("1.25".into())),
            Err(EvalError::TypeMismatch)
        );
        assert_eq!(
            conversion.apply(&Value::Double(Double::from_float(f64::INFINITY))),
            Err(EvalError::NonFinite)
        );
        assert_eq!(
            conversion.apply(&Value::Single(Single::from_float(f32::NAN))),
            Err(EvalError::NonFinite)
        );
    }
    assert_eq!(
        Conversion::Single.apply(&Value::Double(Double::from_float(f64::MAX))),
        Err(EvalError::Overflow)
    );
    let above_max = f64::from_bits(f64::from(f32::MAX).to_bits() + 1);
    assert_eq!(
        Conversion::Single.apply(&Value::Double(Double::from_float(above_max))),
        Err(EvalError::Overflow)
    );
    assert_eq!(
        Conversion::Single.apply(&Value::Double(Double::from_float(-f64::MIN_POSITIVE))),
        Ok(Value::Single(Single::from_float(-0.0)))
    );
    assert_eq!(
        Conversion::Double.apply(&Value::Single(Single::from_float(-0.0))),
        Ok(Value::Double(Double::from_float(-0.0)))
    );
    assert_eq!(
        Conversion::Single.apply(&Value::Double(Double::from_float(f64::from(
            f32::from_bits(1)
        )))),
        Ok(Value::Single(Single::from_bits(1)))
    );
    assert_eq!(
        Conversion::Double.apply(&Value::Int64(9007199254740993)),
        Ok(Value::Double(Double::from_float(9007199254740992.0)))
    );
    assert_eq!(
        crate::numeric::arithmetic(
            crate::numeric::Arithmetic::Add,
            &Value::Int64(9007199254740993),
            &Value::Double(Double::from_float(0.5))
        ),
        Err(EvalError::PrecisionLoss)
    );
    assert_eq!(
        Conversion::Single.apply(&Value::Boolean(true)),
        Ok(Value::Single(Single::from_float(-1.0)))
    );
}
proptest! {
    #![proptest_config(crate::test_support::config())]
    #[test]
    fn finite_single_widening_and_narrowing_preserve_every_bit(bits in any::<u32>()) {
        prop_assume!(f32::from_bits(bits).is_finite());
        let original=Value::Single(Single::from_bits(bits));
        let widened=Conversion::Double.apply(&original).unwrap();
        prop_assert_eq!(Conversion::Single.apply(&widened),Ok(original));
    }
}

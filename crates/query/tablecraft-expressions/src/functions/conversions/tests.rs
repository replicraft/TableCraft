use super::*;
use proptest::prelude::*;
use tablecraft_core::values::{Currency, Decimal, Double, Single};
#[test]
fn conversion_targets_preserve_widths_boolean_exceptions_and_null_errors() {
    assert_eq!(Conversion::resolve("cLnGlNg"), Some(Conversion::LongLong));
    assert_eq!(Conversion::resolve("VBA.CInt"), None);
    for (conversion, expected) in [
        (Conversion::Byte, Value::Byte(2)),
        (Conversion::Integer, Value::Int16(2)),
        (Conversion::Long, Value::Int32(2)),
        (Conversion::LongLong, Value::Int64(2)),
    ] {
        for input in [
            Value::Decimal(Decimal::new(25, 1).unwrap()),
            Value::Currency(Currency::from_scaled(25000)),
            Value::Double(Double::from_float(2.5)),
        ] {
            assert_eq!(conversion.apply(&input), Ok(expected.clone()));
        }
        assert_eq!(
            conversion.apply(&Value::Null),
            Err(EvalError::InvalidNullConversion)
        );
        assert_eq!(
            conversion.apply(&Value::Text("2".into())),
            Err(EvalError::TypeMismatch)
        );
    }
    assert_eq!(
        Conversion::Byte.apply(&Value::Boolean(true)),
        Ok(Value::Byte(255))
    );
    assert_eq!(
        Conversion::Byte.apply(&Value::Boolean(false)),
        Ok(Value::Byte(0))
    );
    assert_eq!(
        Conversion::Byte.apply(&Value::Int16(-1)),
        Err(EvalError::Overflow)
    );
    assert_eq!(
        Conversion::Integer.apply(&Value::Boolean(true)),
        Ok(Value::Int16(-1))
    );
    assert_eq!(
        Conversion::LongLong.apply(&Value::Int64(i64::MAX)),
        Ok(Value::Int64(i64::MAX))
    );
    assert_eq!(
        Conversion::LongLong.apply(&Value::Decimal(Decimal::new(9007199254740993, 0).unwrap())),
        Ok(Value::Int64(9007199254740993))
    );
    assert_eq!(
        Conversion::Boolean.apply(&Value::Null),
        Err(EvalError::InvalidNullConversion)
    );
}
#[test]
fn numeric_truth_is_not_rounded_or_narrowed_and_nonfinite_inputs_fail() {
    for (value, expected) in [
        (Value::Boolean(false), false),
        (Value::Int64(i64::MIN), true),
        (Value::Decimal(Decimal::new(1, 28).unwrap()), true),
        (Value::Double(Double::from_float(f64::from_bits(1))), true),
        (Value::Single(Single::from_float(-0.0)), false),
    ] {
        assert_eq!(
            Conversion::Boolean.apply(&value),
            Ok(Value::Boolean(expected))
        );
    }
    for conversion in [
        Conversion::Byte,
        Conversion::Integer,
        Conversion::Long,
        Conversion::LongLong,
        Conversion::Boolean,
    ] {
        assert_eq!(
            conversion.apply(&Value::Double(Double::from_float(f64::NAN))),
            Err(EvalError::NonFinite)
        );
        assert_eq!(
            conversion.apply(&Value::Binary(vec![])),
            Err(EvalError::TypeMismatch)
        );
    }
    for (conversion, value) in [
        (Conversion::Byte, 255.5),
        (Conversion::Integer, 32767.5),
        (Conversion::Long, 2147483647.5),
        (Conversion::LongLong, 9223372036854775808.0),
    ] {
        assert_eq!(
            conversion.apply(&Value::Double(Double::from_float(value))),
            Err(EvalError::Overflow)
        );
    }
    assert_eq!(
        Conversion::Byte.apply(&Value::Double(Double::from_float(-0.5))),
        Ok(Value::Byte(0))
    );
    assert_eq!(
        Conversion::Integer.apply(&Value::Double(Double::from_float(-1.5))),
        Ok(Value::Int16(-2))
    );
}
proptest! {
    #![proptest_config(crate::test_support::config())]
    #[test]
    fn long_long_identity_and_boolean_truth_keep_every_integer(value in any::<i64>()) {
        let input=Value::Int64(value);
        prop_assert_eq!(Conversion::LongLong.apply(&input),Ok(input.clone()));
        prop_assert_eq!(Conversion::Boolean.apply(&input),Ok(Value::Boolean(value != 0)));
    }
}

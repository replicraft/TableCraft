use super::Conversion;
use crate::{
    EvalError,
    functions::{
        financial::FinancialFunction,
        math::{MathFunction, random::RndState},
    },
};
use proptest::prelude::*;
use tablecraft_core::values::{
    Currency, Decimal, Double, ExtendedDateTime, LegacyDateTime, Single, Value,
};

fn date(serial: f64) -> Value {
    Value::DateTime(LegacyDateTime::from_ole_days(serial).unwrap())
}

#[test]
fn date_conversions_use_the_raw_serial_and_preserve_destination_carriers() {
    for (conversion, expected) in [
        (Conversion::Byte, Value::Byte(2)),
        (Conversion::Integer, Value::Int16(2)),
        (Conversion::Long, Value::Int32(2)),
        (Conversion::LongLong, Value::Int64(2)),
        (Conversion::Boolean, Value::Boolean(true)),
        (Conversion::Single, Value::Single(Single::from_float(2.5))),
        (Conversion::Double, Value::Double(Double::from_float(2.5))),
        (
            Conversion::Currency,
            Value::Currency(Currency::from_scaled(25000)),
        ),
        (
            Conversion::Decimal,
            Value::Decimal(Decimal::new(25, 1).unwrap()),
        ),
    ] {
        assert_eq!(conversion.apply(&date(2.5)), Ok(expected));
        assert_eq!(
            conversion.apply(&Value::Null),
            Err(EvalError::InvalidNullConversion)
        );
        let extended = ExtendedDateTime::new(2025, 1, 1, 0, 0, 0, 1).unwrap();
        assert_eq!(
            conversion.apply(&Value::DateTimeExtended(extended)),
            Err(EvalError::TypeMismatch)
        );
    }
    for serial in [-0.0, 0.0, -0.25, 0.25, -1.25, 45658.5] {
        let Value::Double(result) = Conversion::Double.apply(&date(serial)).unwrap() else {
            panic!("Double required");
        };
        assert_eq!(result.to_float().to_bits(), serial.to_bits());
        assert_eq!(
            Conversion::Boolean.apply(&date(serial)),
            Ok(Value::Boolean(serial != 0.0))
        );
    }
}

#[test]
fn rounding_and_overflow_come_from_the_existing_numeric_destination_rules() {
    for (serial, expected) in [(1.5, 2), (2.5, 2), (3.5, 4), (-1.5, -2), (-2.5, -2)] {
        assert_eq!(
            Conversion::Long.apply(&date(serial)),
            Ok(Value::Int32(expected))
        );
    }
    for (conversion, serial) in [
        (Conversion::Byte, -1.0),
        (Conversion::Byte, 255.5),
        (Conversion::Integer, 32767.5),
        (Conversion::Integer, -32769.0),
        (Conversion::Integer, 45658.5),
    ] {
        assert_eq!(conversion.apply(&date(serial)), Err(EvalError::Overflow));
    }
    assert_eq!(
        Conversion::Currency.apply(&date(2.25)),
        Ok(Value::Currency(Currency::from_scaled(22500)))
    );
    assert_eq!(
        Conversion::Decimal.apply(&date(-1.25)),
        Ok(Value::Decimal(Decimal::new(-125, 2).unwrap()))
    );
}

proptest! {
    #![proptest_config(crate::test_support::config())]

    #[test]
    fn whole_date_serials_preserve_integer_value_and_double_bits(serial in -650_000i32..2_900_000) {
        let value = date(f64::from(serial));
        prop_assert_eq!(
            Conversion::Long.apply(&value),
            Ok(Value::Int32(serial))
        );
        prop_assert_eq!(
            Conversion::LongLong.apply(&value),
            Ok(Value::Int64(i64::from(serial)))
        );
        let Value::Double(result) = Conversion::Double.apply(&value).unwrap() else {
            panic!("Double required");
        };
        prop_assert_eq!(
            result.to_float().to_bits(),
            f64::from(serial).to_bits()
        );
    }
}

#[test]
fn typed_numeric_consumers_share_date_coercion_and_keep_domain_and_state_rules() {
    assert_eq!(
        MathFunction::Sqr.apply(&[date(4.0)]),
        Ok(Value::Double(Double::from_float(2.0)))
    );
    assert_eq!(
        MathFunction::Log.apply(&[date(0.0)]),
        Err(EvalError::Domain)
    );
    assert_eq!(
        FinancialFunction::SLN.apply(&[date(1000.0), date(0.0), date(5.0)]),
        Ok(Value::Double(Double::from_float(200.0)))
    );
    let state = RndState::default();
    assert_eq!(
        state.apply(Some(&date(-1.0))),
        Ok(Value::Single(Single::from_float(0.22400701)))
    );
}

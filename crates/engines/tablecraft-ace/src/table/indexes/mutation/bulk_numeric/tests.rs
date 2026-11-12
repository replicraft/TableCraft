//! Literal physical keys are independent from population, profile and sort code.
use super::NumericKey;
use crate::{InspectError, table::indexes::numeric::encode_component};
use tablecraft_core::values::{Currency, Double, LegacyDateTime, Single, Value};
use tablecraft_schema::ScalarType;

#[test]
fn literal_numeric_keys_preserve_boolean_order_scaled_integers_and_ieee_bits() {
    for (scalar, value, atom, ascending) in [
        (
            ScalarType::Boolean,
            Value::Boolean(true),
            -1,
            &[0x7f, 0x00][..],
        ),
        (
            ScalarType::Boolean,
            Value::Boolean(false),
            0,
            &[0x7f, 0xff][..],
        ),
        (ScalarType::Byte, Value::Byte(255), 255, &[0x7f, 0xff][..]),
        (
            ScalarType::Int16,
            Value::Int16(-1),
            -1,
            &[0x7f, 0x7f, 0xff][..],
        ),
        (
            ScalarType::Int32,
            Value::Int32(i32::MIN),
            i64::from(i32::MIN),
            &[0x7f, 0, 0, 0, 0][..],
        ),
        (
            ScalarType::Int64,
            Value::Int64(i64::MIN),
            i64::MIN,
            &[0x7f, 0, 0, 0, 0, 0, 0, 0, 0][..],
        ),
        (
            ScalarType::Currency,
            Value::Currency(Currency::from_scaled(9_007_199_254_740_993)),
            9_007_199_254_740_993,
            &[0x7f, 0x80, 0x20, 0, 0, 0, 0, 0, 1][..],
        ),
        (
            ScalarType::Single,
            Value::Single(Single::from_bits(0x8000_0000)),
            -1,
            &[0x7f, 0x7f, 0xff, 0xff, 0xff][..],
        ),
        (
            ScalarType::Single,
            Value::Single(Single::from_bits(0)),
            0,
            &[0x7f, 0x80, 0, 0, 0][..],
        ),
        (
            ScalarType::Double,
            Value::Double(Double::from_bits(0x8000_0000_0000_0000)),
            -1,
            &[0x7f, 0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff][..],
        ),
        (
            ScalarType::Double,
            Value::Double(Double::from_bits(0)),
            0,
            &[0x7f, 0x80, 0, 0, 0, 0, 0, 0, 0][..],
        ),
        (
            ScalarType::DateTime,
            Value::DateTime(LegacyDateTime::from_bits(0x4004_0000_0000_0000).unwrap()),
            0x4004_0000_0000_0000,
            &[0x7f, 0xc0, 0x04, 0, 0, 0, 0, 0, 0][..],
        ),
    ] {
        let kind = NumericKey::new(scalar).unwrap();
        assert_eq!(kind.value(&value).unwrap(), Some(atom));
        let mut wire = [0; 17];
        let length = encode_component(Some(atom), kind, false, &mut wire).unwrap();
        assert_eq!(&wire[..length], ascending, "{scalar:?}");
        let length = encode_component(Some(atom), kind, true, &mut wire).unwrap();
        let descending = ascending.iter().map(|byte| !byte).collect::<Vec<_>>();
        assert_eq!(&wire[..length], descending, "{scalar:?}");
    }
}

#[test]
fn null_and_wrong_carrier_rules_remain_specific_to_the_actual_numeric_kind() {
    for scalar in [
        ScalarType::Byte,
        ScalarType::Int16,
        ScalarType::Int32,
        ScalarType::Int64,
        ScalarType::Currency,
        ScalarType::Single,
        ScalarType::Double,
        ScalarType::DateTime,
    ] {
        let kind = NumericKey::new(scalar).unwrap();
        assert_eq!(kind.value(&Value::Null).unwrap(), None);
        for (descending, marker) in [(false, 0x00), (true, 0xff)] {
            let mut wire = [0; 17];
            assert_eq!(
                encode_component(None, kind, descending, &mut wire).unwrap(),
                1
            );
            assert_eq!(wire[0], marker);
        }
    }
    let boolean = NumericKey::new(ScalarType::Boolean).unwrap();
    assert!(matches!(
        boolean.value(&Value::Null),
        Err(InspectError::Malformed)
    ));
    assert!(matches!(
        boolean.value(&Value::Int32(-1)),
        Err(InspectError::Malformed)
    ));
    assert!(matches!(
        NumericKey::new(ScalarType::Currency)
            .unwrap()
            .value(&Value::Int64(0)),
        Err(InspectError::Malformed)
    ));
    assert!(matches!(
        encode_component(None, boolean, false, &mut [0; 17]),
        Err(InspectError::Malformed)
    ));
}

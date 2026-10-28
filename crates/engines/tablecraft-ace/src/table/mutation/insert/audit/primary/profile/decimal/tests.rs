//! Literal key magnitudes differ deliberately from FIELD limb endianness.
use super::super::tests::with_reader;
use super::*;
use tablecraft_core::values::{Currency, Decimal};

#[test]
fn decimal_literal_sign_magnitude_and_direction_preserve_exact_scale_and_row_bytes() {
    with_reader(|reader| {
        for (coefficient, ascending) in [
            (
                0,
                vec![127, 255, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            ),
            (
                1,
                vec![127, 255, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
            ),
            (
                -1,
                vec![
                    127, 0, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 254,
                ],
            ),
            (
                0x0102030405060708090a0b0c,
                vec![127, 255, 0, 0, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12],
            ),
        ] {
            for descending in [false, true] {
                let profile = Profile::new(28, 4, descending).unwrap();
                let value = Value::Decimal(Decimal::new(coefficient, 4).unwrap());
                let wire = profile.encode(&value, 0x01020304, reader).unwrap();
                let mut expected = ascending.clone();
                if descending {
                    for byte in &mut expected {
                        *byte = !*byte;
                    }
                }
                expected.extend([1, 2, 3, 4]);
                assert_eq!(&wire[..], expected);
                assert_eq!(profile.validate(&wire).unwrap(), (false, 18));
                assert!(matches!(wire, Wire::Owned(_)));

                let null = profile.encode(&Value::Null, 0x01020304, reader).unwrap();
                assert_eq!(&null[..], &[if descending { 255 } else { 0 }, 1, 2, 3, 4]);
                assert_eq!(profile.validate(&null).unwrap(), (true, 1));
            }
        }
    });
}

#[test]
fn decimal_declarations_carriers_precision_sign_and_negative_zero_are_closed() {
    with_reader(|reader| {
        for (precision, scale) in [(0, 0), (29, 0), (2, 3)] {
            assert!(matches!(
                Profile::new(precision, scale, false),
                Err(EditError::Preservation)
            ));
        }
        for (precision, scale) in [(1, 0), (28, 28)] {
            let profile = Profile::new(precision, scale, false).unwrap();
            let value = Value::Decimal(Decimal::new(9, scale).unwrap());
            let encoded = profile.encode(&value, 1, reader).unwrap();
            assert_eq!(profile.validate(&encoded).unwrap(), (false, 18));
        }
        let profile = Profile::new(3, 2, false).unwrap();
        for value in [
            Value::Decimal(Decimal::new(1, 1).unwrap()),
            Value::Decimal(Decimal::new(1000, 2).unwrap()),
            Value::Decimal(Decimal::new(-1000, 2).unwrap()),
            Value::Currency(Currency::from_scaled(1)),
            Value::Int64(1),
        ] {
            assert!(matches!(
                profile.encode(&value, 1, reader),
                Err(EditError::Preservation)
            ));
        }
        for ascending in [
            vec![127, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
            vec![
                127, 0, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                255,
            ],
            vec![127, 255, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 232],
        ] {
            for descending in [false, true] {
                let mut encoded = ascending.clone();
                if descending {
                    for byte in &mut encoded {
                        *byte = !*byte;
                    }
                }
                let profile = Profile::new(3, 2, descending).unwrap();
                assert!(matches!(
                    profile.validate(&encoded),
                    Err(EditError::Preservation)
                ));
            }
        }
    });
}

use super::Number;
use crate::CreateDatabaseError;
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Decimal, Double, Single, Value},
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(0, 0, 100_000)
}

#[test]
fn finite_float_literals_are_bounded_scientific_exact_bit_roundtrips() {
    for bits in [
        0_u32,
        0x8000_0000,
        1,
        0x8000_0001,
        0x007f_ffff,
        0x0080_0000,
        0x3f80_0000,
        0x7f7f_ffff,
        0xff7f_ffff,
    ] {
        let value = Value::Single(Single::from_bits(bits));
        let literal = Number::capture(&value, &mut budget(), &|| false).unwrap();
        let text = std::str::from_utf8(literal.bytes()).unwrap();
        assert!(text.contains('e'));
        assert!(text.len() <= 24);
        assert_eq!(text.parse::<f32>().unwrap().to_bits(), bits);
    }
    for bits in [
        0_u64,
        0x8000_0000_0000_0000,
        1,
        0x8000_0000_0000_0001,
        0x000f_ffff_ffff_ffff,
        0x0010_0000_0000_0000,
        0x3ff0_0000_0000_0000,
        0x7fef_ffff_ffff_ffff,
        0xffef_ffff_ffff_ffff,
    ] {
        let value = Value::Double(Double::from_bits(bits));
        let literal = Number::capture(&value, &mut budget(), &|| false).unwrap();
        let text = std::str::from_utf8(literal.bytes()).unwrap();
        assert!(text.contains('e'));
        assert!(text.len() <= 32);
        assert_eq!(text.parse::<f64>().unwrap().to_bits(), bits);
    }
    for value in [
        Value::Single(Single::from_bits(0x7f80_0000)),
        Value::Single(Single::from_bits(0x7fc0_0001)),
        Value::Double(Double::from_bits(0xfff0_0000_0000_0000)),
        Value::Double(Double::from_bits(0x7ff8_0000_0000_0001)),
    ] {
        assert!(matches!(
            Number::capture(&value, &mut budget(), &|| false),
            Err(CreateDatabaseError::Carrier)
        ));
    }
}

#[test]
fn exact_decimal_text_keeps_declared_scale_without_rounding() {
    for (coefficient, scale, expected) in [
        (0, 0, "0"),
        (0, 4, "0.0000"),
        (-123450, 4, "-12.3450"),
        (1, 28, "0.0000000000000000000000000001"),
        (
            -9999999999999999999999999999_i128,
            0,
            "-9999999999999999999999999999",
        ),
    ] {
        let value = Value::Decimal(Decimal::new(coefficient, scale).unwrap());
        let literal = Number::capture(&value, &mut budget(), &|| false).unwrap();
        assert_eq!(std::str::from_utf8(literal.bytes()).unwrap(), expected);
    }
    assert!(Number::capture(&Value::Byte(1), &mut budget(), &|| false).is_err());
}

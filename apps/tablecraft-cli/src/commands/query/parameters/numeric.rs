use super::Error;
use tablecraft_core::values::{Decimal, Double, Single, Value};

pub(super) fn decimal(text: &str) -> Result<Value, Error> {
    let unsigned = text.strip_prefix(['+', '-']).unwrap_or(text);
    let (integer, fraction) = unsigned.split_once('.').unwrap_or((unsigned, ""));
    if integer.is_empty() && fraction.is_empty()
        || !integer.bytes().all(|byte| byte.is_ascii_digit())
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
        || fraction.len() > usize::from(Decimal::MAX_SCALE)
    {
        return Err(Error::Parameter);
    }

    let parsed = rust_decimal::Decimal::from_str_exact(text).map_err(|_| Error::Parameter)?;
    let scale = u8::try_from(parsed.scale()).map_err(|_| Error::Parameter)?;
    if usize::from(scale) != fraction.len() {
        return Err(Error::Parameter);
    }

    let value = Decimal::new(parsed.mantissa(), scale).map_err(|_| Error::Parameter)?;
    Ok(Value::Decimal(value))
}

pub(super) fn single(text: &str) -> Result<Value, Error> {
    let value: f32 = text.parse().map_err(|_| Error::Parameter)?;
    if !value.is_finite() {
        return Err(Error::Parameter);
    }

    Ok(Value::Single(Single::from_float(value)))
}

pub(super) fn double(text: &str) -> Result<Value, Error> {
    let value: f64 = text.parse().map_err(|_| Error::Parameter)?;
    if !value.is_finite() {
        return Err(Error::Parameter);
    }

    Ok(Value::Double(Double::from_float(value)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decimal_retains_exact_coefficient_and_scale() {
        for (text, coefficient, scale) in [
            ("1.2300", 12300, 4),
            ("-0.000", 0, 3),
            ("+42", 42, 0),
            (".5", 5, 1),
            ("1.", 1, 0),
            ("0.0000000000000000000000000001", 1, 28),
            (
                "79228162514264337593543950335",
                79228162514264337593543950335,
                0,
            ),
            (
                "-79228162514264337593543950335",
                -79228162514264337593543950335,
                0,
            ),
        ] {
            let Value::Decimal(value) = decimal(text).ok().expect("valid numeric parameter") else {
                panic!("expected Decimal");
            };
            assert_eq!(value.coefficient(), coefficient);
            assert_eq!(value.scale(), scale);
        }

        for text in [
            "",
            ".",
            "+",
            "-",
            "1_000",
            "1e2",
            " 1",
            "1 ",
            "1,5",
            "1.2.3",
            "79228162514264337593543950336",
            "79228162514264337593543950335.0",
            "0.00000000000000000000000000001",
            "0.00000000000000000000000000000",
        ] {
            assert!(decimal(text).is_err(), "accepted {text:?}");
        }
    }

    #[test]
    fn float_parsing_preserves_signed_zero_and_subnormals() {
        assert_eq!(
            single("-0").ok().expect("valid numeric parameter"),
            Value::Single(Single::from_bits(0x80000000))
        );
        assert_eq!(
            double("-0").ok().expect("valid numeric parameter"),
            Value::Double(Double::from_bits(0x8000000000000000))
        );
        assert_eq!(
            single("1.401298464324817e-45")
                .ok()
                .expect("valid numeric parameter"),
            Value::Single(Single::from_bits(1))
        );
        assert_eq!(
            double("4.9406564584124654e-324")
                .ok()
                .expect("valid numeric parameter"),
            Value::Double(Double::from_bits(1))
        );

        for text in ["NaN", "inf", "-inf", "1e1000", "1_000", " 1", "1,5", ""] {
            assert!(single(text).is_err(), "accepted Single {text:?}");
            assert!(double(text).is_err(), "accepted Double {text:?}");
        }
        assert!(single("1e39").is_err());
        assert!(double("1e39").is_ok());
    }
}

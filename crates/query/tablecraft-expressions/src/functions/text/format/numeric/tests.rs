use super::super::tests::render;
use crate::{EvalError, functions::conversions::text::NumberFormat};
use proptest::prelude::*;
use tablecraft_core::values::{Decimal, Double, Value};

#[test]
fn scientific_exponent_sign_width_case_and_carry_are_explicit() {
    for (value, mask, expected) in [
        (Value::Int32(12345), "0.00E+00", "1.23E+04"),
        (Value::Int32(12345), "0.00e-000", "1.23e004"),
        (
            Value::Decimal(Decimal::new(123, 5).unwrap()),
            "0.0E+0",
            "1.2E-3",
        ),
        (
            Value::Decimal(Decimal::new(9995, 3).unwrap()),
            "0.00E+00",
            "1.00E+01",
        ),
        (Value::Int32(12), "000.0E+00", "120.0E-01"),
        (Value::Int32(-12), "0.00E+00;(0.00E+00)", "(1.20E+01)"),
        (Value::Int32(0), "0.00E+00", "0.00E+00"),
        (Value::Int32(12345), "Scientific", "1.23E+04"),
        (Value::Int32(12345), "0.00E+#", "1.23E+4"),
        (Value::Int32(12000), "0.###E+00", "1.2E+04"),
        (
            Value::Double(Double::from_float(f64::MAX)),
            "0.00E+0",
            "1.80E+308",
        ),
        (
            Value::Double(Double::from_float(f64::from_bits(1))),
            "0.00E+0",
            "4.94E-324",
        ),
    ] {
        assert_eq!(
            render(value, mask, Some(NumberFormat::EnUs)),
            Ok(expected.into()),
            "{mask}"
        );
    }
    assert_eq!(
        render(Value::Int32(12345), "0.00E+00", Some(NumberFormat::DeDe)),
        Ok("1,23E+04".into())
    );
    for mask in ["0E+", "0E0", "0E++00", "0E+00E-0", ".00E+00"] {
        assert_eq!(
            render(Value::Int32(1), mask, Some(NumberFormat::EnUs)),
            Err(EvalError::UnsupportedExpression),
            "{mask}"
        );
    }
}

#[test]
fn scaling_commas_do_not_confuse_grouping_percent_literals_or_tiny_magnitudes() {
    for (value, mask, expected) in [
        (Value::Int32(100_000_000), "##0,,", "100"),
        (Value::Int32(123456), "0,.00", "123.46"),
        (Value::Int32(123456), "0,.00%", "12345.60%"),
        (Value::Int32(123456), "#,##0,.00", "123.46"),
        (Value::Int32(1234567), "#,##0,.00", "1,234.57"),
        (Value::Int32(1234567), "#,,##0", "1,234,567"),
        (Value::Int32(123456), r"0\,", "123456,"),
        (Value::Int32(123456), "0,.00E+00", "1.23E+02"),
        (
            Value::Decimal(Decimal::new(1, 28).unwrap()),
            "0,.0000000000000000000000000000000",
            "0.0000000000000000000000000000001",
        ),
        (Value::Int32(-1), "0,,", "0"),
    ] {
        assert_eq!(
            render(value, mask, Some(NumberFormat::EnUs)),
            Ok(expected.into()),
            "{mask}"
        );
    }
}

#[test]
fn rounding_at_the_decimal_carrier_boundary_remains_display_only() {
    let below = Decimal::new(49_999_999_999_999_999_999_999_999_999, 1).unwrap();
    let tie = Decimal::new(50_000_000_000_000_000_000_000_000_000, 1).unwrap();
    let maximum = Decimal::new(79_228_162_514_264_337_593_543_950_335, 0).unwrap();
    let mask = "0".to_owned() + &",".repeat(10) + "%";
    // Net scaling is -28 places: this retains the side of the exact half.
    assert_eq!(
        render(Value::Decimal(below), &mask, Some(NumberFormat::EnUs)),
        Ok("0%".into())
    );
    assert_eq!(
        render(Value::Decimal(tie), &mask, Some(NumberFormat::EnUs)),
        Ok("1%".into())
    );
    assert_eq!(
        render(Value::Decimal(maximum), "0E+00", Some(NumberFormat::EnUs)),
        Ok("8E+28".into())
    );
    // A one-place mantissa can round above Decimal::MAX without storing that value.
    assert_eq!(
        render(Value::Decimal(maximum), "0,.00", Some(NumberFormat::EnUs)),
        Ok("79228162514264337593543950.34".into())
    );
}

proptest! {
    #![proptest_config(crate::test_support::config())]

    #[test]
    fn integer_scientific_masks_match_independent_digit_exponents(value in 1u32..1_000_000, width in 1usize..5) {
        let digits = value.to_string();
        let exponent = digits.len() as i32 - width as i32;
        let mantissa = if digits.len() >= width {
            let mut text = digits[..width].to_owned();
            text.push('.');
            text.push_str(&digits[width..]);
            text.push_str(&"0".repeat(6 - (digits.len() - width)));
            text
        } else {
            digits.clone() + &"0".repeat(width - digits.len()) + ".000000"
        };
        let mask = "0".repeat(width) + ".000000E+00";
        let expected = format!("{mantissa}E{exponent:+03}");
        prop_assert_eq!(render(Value::Int64(i64::from(value)), &mask, Some(NumberFormat::EnUs)).unwrap(), expected);
    }
}

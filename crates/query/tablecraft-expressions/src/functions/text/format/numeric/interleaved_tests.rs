use super::super::tests::render;
use crate::{EvalError, functions::conversions::text::NumberFormat};
use proptest::prelude::*;
use tablecraft_core::values::{Currency, Decimal, Double, Value};

#[test]
fn literal_positions_do_not_consume_digits_or_change_numeric_rounding() {
    for (value, mask, expected) in [
        (Value::Int32(12), "00-00", "00-12"),
        (Value::Int32(12345), "00-00", "123-45"),
        (Value::Int32(7), "\"ID \"00\"🦀; \"00\"!\"", "ID 00🦀; 07!"),
        (Value::Int32(0), "#\"x\"#0", "x0"),
        (Value::Int32(12), "0%0", "120%0"),
        (Value::Int32(12), r"0\%0", "1%2"),
        (
            Value::Decimal(Decimal::new(12345, 3).unwrap()),
            "0.0\"x\"0",
            "12.3x5",
        ),
        (
            Value::Currency(Currency::from_scaled(1250)),
            "0.0%0",
            "12.5%0",
        ),
        (Value::Int32(7), ".0\"x\"0", "7.0x0"),
        (Value::Int32(0), "0.#\"x\"#", "0.x"),
        (Value::Int32(12345), "0\"x\".00E+00", "1x.23E+04"),
        (Value::Int32(12345), "0.00E+0\"x\"0", "1.23E+0x4"),
        (
            Value::Decimal(Decimal::new(123, 5).unwrap()),
            "0.00e-0\"x\"0",
            "1.23e-0x3",
        ),
        (
            Value::Double(Double::from_float(f64::MAX)),
            "0.0E+0\"x\"0",
            "1.8E+30x8",
        ),
        (Value::Int32(0), "0.0E+0\"x\"0", "0.0E+0x0"),
        (Value::Int32(1234567), "#,##0\"u\".00", "1,234,567u.00"),
        (Value::Int32(-12), "00-00;(00-00)", "(00-12)"),
        (Value::Int32(0), "00-00;(00-00);\"zero\";\"null\"", "zero"),
        (Value::Null, "00-00;(00-00);\"zero\";\"null\"", "null"),
    ] {
        assert_eq!(
            render(value, mask, Some(NumberFormat::EnUs)),
            Ok(expected.into()),
            "{mask}"
        );
    }
    assert_eq!(
        render(
            Value::Decimal(Decimal::new(1234, 2).unwrap()),
            "0.0\"x\"0",
            Some(NumberFormat::DeDe)
        ),
        Ok("12,3x4".into())
    );
    assert_eq!(
        render(Value::Int32(1), "0-0", None),
        Err(EvalError::NumberFormatRequired)
    );
}

#[test]
fn required_whole_positions_retain_the_optional_slots_to_their_right() {
    for (value, mask, expected) in [
        (1, "0#", "01"),
        (0, "0#", "00"),
        (1, "#0\"x\"#", "0x1"),
        (1, "0\"x\"##", "0x01"),
        (12345, "0#", "12345"),
        (0, "##", ""),
        (1234, "0#,#0", "1,234"),
    ] {
        assert_eq!(
            render(Value::Int32(value), mask, Some(NumberFormat::EnUs)),
            Ok(expected.into()),
            "{mask}"
        );
    }
}

proptest! {
    #![proptest_config(crate::test_support::config())]

    #[test]
    fn whole_literal_layout_matches_right_aligned_decimal_positions(
        value in 0u64..10_000_000_000,
        left in 1usize..5,
        right in 1usize..5,
    ) {
        let width = left + right;
        let padded = format!("{value:0width$}");
        let boundary = padded.len() - right;
        let expected = format!("{}🦀{}", &padded[..boundary], &padded[boundary..]);
        let mask = "0".repeat(left) + "\"🦀\"" + &"0".repeat(right);
        prop_assert_eq!(
            render(Value::Int64(value as i64), &mask, Some(NumberFormat::EnUs)),
            Ok(expected)
        );
    }
}

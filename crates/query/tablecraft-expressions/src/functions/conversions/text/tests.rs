use super::*;
use crate::functions::text::output::TextOutput;
use proptest::prelude::*;
use tablecraft_core::values::Decimal;
struct Buffer {
    text: String,
    steps: usize,
    bytes: usize,
}
impl TextOutput for Buffer {
    fn step(&mut self) -> Result<(), EvalError> {
        self.steps = self.steps.checked_sub(1).ok_or(EvalError::Cancelled)?;
        Ok(())
    }
    fn append(&mut self, text: &str) -> Result<(), EvalError> {
        self.step()?;
        if self.text.len() + text.len() > self.bytes {
            return Err(EvalError::TextLimit);
        }
        self.text.push_str(text);
        Ok(())
    }
}
impl NumericBuffer for Buffer {
    fn text(&self) -> &str {
        &self.text
    }
}
fn bounded(
    conversion: Conversion,
    text: &str,
    format: NumberFormat,
    steps: usize,
    bytes: usize,
) -> Result<Value, EvalError> {
    conversion.apply_text(
        text,
        format,
        &mut Buffer {
            text: String::new(),
            steps,
            bytes,
        },
    )
}
fn call(conversion: Conversion, text: &str, format: NumberFormat) -> Result<Value, EvalError> {
    bounded(conversion, text, format, 100_000, 65_536)
}
#[test]
fn profiles_parse_entire_grouped_currency_and_exponent_spellings() {
    for (format, text, number) in [
        (NumberFormat::Invariant, " + 1 . 25 D - 2 ", 0.0125),
        (NumberFormat::EnUs, "$1,234.50", 1234.5),
        (NumberFormat::EnUs, "( $ 1,234.50 )", -1234.5),
        (NumberFormat::EnUs, "-$1,234.50", -1234.5),
        (NumberFormat::EnUs, "$-1,234.50", -1234.5),
        (NumberFormat::EnUs, "1 , 234 . 50 $", 1234.5),
        (NumberFormat::DeDe, "1.234,50 €", 1234.5),
        (NumberFormat::DeDe, "€ -1.234,50e+1", -12345.0),
        (NumberFormat::Invariant, ".5", 0.5),
    ] {
        assert_eq!(
            call(Conversion::Double, text, format),
            Ok(Value::Double(Double::from_float(number))),
            "{text}"
        );
    }
    for format in [
        NumberFormat::Invariant,
        NumberFormat::EnUs,
        NumberFormat::DeDe,
    ] {
        for text in [
            "12tail", "1e", "1e+", "1e2e3", "1 2", "1%", "&HFF", "--1", "", ".", "$€1", "(1", "1)",
        ] {
            assert_eq!(
                call(Conversion::Double, text, format),
                Err(EvalError::TypeMismatch),
                "{format:?}:{text}"
            );
        }
    }
    for text in [
        "1,23.0", "1234,567", "1,,234", "1,234,56", "1.2,3", "$1$", "1.2.3",
    ] {
        assert_eq!(
            call(Conversion::Double, text, NumberFormat::EnUs),
            Err(EvalError::TypeMismatch),
            "{text}"
        );
    }
}
#[test]
fn exact_integer_and_currency_rounding_does_not_double_round() {
    for (text, expected) in [
        ("1.49999999999999999999999999999", 1),
        ("1.50000000000000000000000000001", 2),
        ("2.5", 2),
        ("-2.5", -2),
        ("-3.5", -4),
        ("0e99999999999999999999999", 0),
        ("1e-99999999999999999999999", 0),
    ] {
        assert_eq!(
            call(Conversion::LongLong, text, NumberFormat::Invariant),
            Ok(Value::Int64(expected)),
            "{text}"
        );
    }
    for (text, coefficient) in [
        ("1.23444999999999999999999999999", 12344),
        ("1.23445000000000000000000000001", 12345),
        ("-922337203685477.5808", i64::MIN),
        ("922337203685477.5807", i64::MAX),
        ("1e-999999999999999999999", 0),
    ] {
        assert_eq!(
            call(Conversion::Currency, text, NumberFormat::Invariant),
            Ok(Value::Currency(Currency::from_scaled(coefficient))),
            "{text}"
        );
    }
    assert_eq!(
        call(Conversion::Integer, "32767.5", NumberFormat::Invariant),
        Err(EvalError::Overflow)
    );
    assert_eq!(
        call(
            Conversion::LongLong,
            "9223372036854775807",
            NumberFormat::Invariant
        ),
        Ok(Value::Int64(i64::MAX))
    );
    assert_eq!(
        call(
            Conversion::LongLong,
            "-9223372036854775808",
            NumberFormat::Invariant
        ),
        Ok(Value::Int64(i64::MIN))
    );
    assert_eq!(
        call(
            Conversion::Currency,
            "922337203685477.5808",
            NumberFormat::Invariant
        ),
        Err(EvalError::Overflow)
    );
}
#[test]
fn decimal_text_fits_shared_magnitude_scale_and_direct_rounding() {
    for (text, coefficient, scale) in [
        ("1.2300", 12300, 4),
        ("1e-28", 1, 28),
        ("1e-29", 0, 28),
        ("79228162514264337593543950335", Decimal::MAX_COEFFICIENT, 0),
        (
            "7922816251426433759354395033.6",
            7922816251426433759354395034,
            0,
        ),
        (
            "1.00000000000000000000000000015",
            10000000000000000000000000002,
            28,
        ),
        (
            "1.00000000000000000000000000005",
            10000000000000000000000000000,
            28,
        ),
    ] {
        assert_eq!(
            call(Conversion::Decimal, text, NumberFormat::Invariant),
            Ok(Value::Decimal(Decimal::new(coefficient, scale).unwrap())),
            "{text}"
        );
    }
    assert_eq!(
        call(
            Conversion::Decimal,
            "79228162514264337593543950336",
            NumberFormat::Invariant
        ),
        Err(EvalError::Overflow)
    );
}
#[test]
fn floating_boolean_and_resource_policies_are_explicit() {
    for text in ["true", "TrUe", "#TRUE#", "1"] {
        assert_eq!(
            call(Conversion::Boolean, text, NumberFormat::Invariant),
            Ok(Value::Boolean(true))
        );
    }
    for text in ["false", "#FALSE#", "-0", "1e-999"] {
        assert_eq!(
            call(Conversion::Boolean, text, NumberFormat::Invariant),
            Ok(Value::Boolean(false))
        );
    }
    assert_eq!(
        call(Conversion::Boolean, "#true#", NumberFormat::Invariant),
        Err(EvalError::TypeMismatch)
    );
    assert_eq!(
        call(
            Conversion::Single,
            "1.0000000596046447753906250001",
            NumberFormat::Invariant
        ),
        Ok(Value::Single(Single::from_float(f32::from_bits(
            1.0_f32.to_bits() + 1
        ))))
    );
    assert_eq!(
        call(
            Conversion::Single,
            "1.000000059604644775390625",
            NumberFormat::Invariant
        ),
        Ok(Value::Single(Single::from_float(1.0)))
    );
    assert_eq!(
        call(Conversion::Double, "-0", NumberFormat::Invariant),
        Ok(Value::Double(Double::from_float(-0.0)))
    );
    assert_eq!(
        call(Conversion::Double, "1e999", NumberFormat::Invariant),
        Err(EvalError::Overflow)
    );
    assert_eq!(
        call(Conversion::Single, "1e40", NumberFormat::Invariant),
        Err(EvalError::Overflow)
    );
    assert_eq!(
        bounded(Conversion::Long, "1234", NumberFormat::Invariant, 100, 3),
        Err(EvalError::TextLimit)
    );
    assert_eq!(
        bounded(Conversion::Long, "     1", NumberFormat::Invariant, 3, 10),
        Err(EvalError::Cancelled)
    );
    assert_eq!(
        bounded(Conversion::Long, "1234", NumberFormat::Invariant, 100, 4),
        Ok(Value::Int32(1234))
    );
}
proptest! {
    #![proptest_config(crate::test_support::config())]
    #[test]
    fn integer_extrema_never_pass_through_floating_point(number in any::<i64>()) {
        prop_assert_eq!(call(Conversion::LongLong,&number.to_string(),NumberFormat::Invariant),Ok(Value::Int64(number)));
    }
}

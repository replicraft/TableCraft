//! Pure packed Access color numbers, independent of platform/UI color objects.
use super::MathFunction;
use crate::{EvalError, functions::conversions::Conversion};
use tablecraft_core::values::Value;

const CHANNEL_MAX: i32 = u8::MAX as i32;
const GREEN_SHIFT: u32 = u8::BITS;
const BLUE_SHIFT: u32 = 2 * u8::BITS;
const BASIC_PALETTE: [i32; 16] = [
    0x000000, 0x800000, 0x008000, 0x808000, 0x000080, 0x800080, 0x008080, 0xc0c0c0, 0x808080,
    0xff0000, 0x00ff00, 0xffff00, 0x0000ff, 0xff00ff, 0x00ffff, 0xffffff,
];

fn number(
    value: &Value,
    convert: &mut impl FnMut(Conversion, &Value) -> Result<Value, EvalError>,
) -> Result<i32, EvalError> {
    let Value::Int32(value) = convert(Conversion::Long, value)? else {
        return Err(EvalError::TypeMismatch);
    };
    if value < 0 {
        return Err(EvalError::Domain);
    }
    Ok(value)
}

pub(super) fn apply_with_input<'a>(
    function: MathFunction,
    argument: &mut impl FnMut(usize) -> Result<&'a Value, EvalError>,
    convert: &mut impl FnMut(Conversion, &Value) -> Result<Value, EvalError>,
) -> Result<Value, EvalError> {
    let packed = match function {
        MathFunction::Rgb => {
            let red = number(argument(0)?, convert)?.min(CHANNEL_MAX);
            let green = number(argument(1)?, convert)?.min(CHANNEL_MAX);
            let blue = number(argument(2)?, convert)?.min(CHANNEL_MAX);

            red | (green << GREEN_SHIFT) | (blue << BLUE_SHIFT)
        }
        MathFunction::QbColor => {
            let Value::Int16(index) = convert(Conversion::Integer, argument(0)?)? else {
                return Err(EvalError::TypeMismatch);
            };
            let index = usize::try_from(index).map_err(|_| EvalError::Domain)?;
            *BASIC_PALETTE.get(index).ok_or(EvalError::Domain)?
        }
        _ => return Err(EvalError::UnsupportedExpression),
    };

    Ok(Value::Int32(packed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use tablecraft_core::values::{Currency, Decimal, Double, Single};

    #[test]
    fn access_palette_and_channel_order_match_independent_goldens() {
        // Decimal color numbers are independent of the production hex palette.
        let expected = [
            0, 8388608, 32768, 8421376, 128, 8388736, 32896, 12632256, 8421504, 16711680, 65280,
            16776960, 255, 16711935, 65535, 16777215,
        ];
        for (index, expected) in expected.into_iter().enumerate() {
            assert_eq!(
                MathFunction::QbColor.apply(&[Value::Byte(index as u8)]),
                Ok(Value::Int32(expected))
            );
        }

        assert_eq!(
            MathFunction::Rgb.apply(&[Value::Byte(1), Value::Byte(2), Value::Byte(3)]),
            Ok(Value::Int32(197121))
        );
        assert_eq!(
            MathFunction::Rgb.apply(&[
                Value::Int32(256),
                Value::Int32(999),
                Value::Int32(i32::MAX)
            ]),
            Ok(Value::Int32(16777215))
        );
    }

    #[test]
    fn numeric_conversion_rounds_before_checked_palette_or_channel_selection() {
        for value in [
            Value::Double(Double::from_float(2.5)),
            Value::Single(Single::from_float(2.5)),
            Value::Decimal(Decimal::new(25, 1).unwrap()),
            Value::Currency(Currency::from_scaled(25_000)),
        ] {
            assert_eq!(
                MathFunction::QbColor.apply(&[value]),
                Ok(Value::Int32(32768))
            );
        }

        for (value, error) in [
            (Value::Null, EvalError::InvalidNullConversion),
            (Value::Boolean(true), EvalError::Domain),
            (Value::Int32(-1), EvalError::Domain),
            (Value::Int32(16), EvalError::Domain),
            (Value::Text("2".into()), EvalError::TypeMismatch),
            (Value::Int64(i64::MAX), EvalError::Overflow),
        ] {
            assert_eq!(MathFunction::QbColor.apply(&[value]), Err(error));
        }
    }

    #[test]
    fn palette_arguments_use_integer_conversion_before_domain_validation() {
        for value in [
            Value::Int32(32768),
            Value::Int32(-32769),
            Value::Double(Double::from_float(32767.5)),
            Value::Decimal(Decimal::new(327675, 1).unwrap()),
            Value::Currency(Currency::from_scaled(327675000)),
        ] {
            assert_eq!(
                MathFunction::QbColor.apply(&[value]),
                Err(EvalError::Overflow)
            );
        }

        for value in [
            Value::Int16(32767),
            Value::Int16(-32768),
            Value::Double(Double::from_float(32766.5)),
        ] {
            assert_eq!(
                MathFunction::QbColor.apply(&[value]),
                Err(EvalError::Domain)
            );
        }

        // RGB keeps its separately documented Long/saturation profile.
        assert_eq!(
            MathFunction::Rgb.apply(&[Value::Int32(32768), Value::Byte(0), Value::Byte(0)]),
            Ok(Value::Int32(255))
        );
    }

    #[test]
    fn three_channel_arity_does_not_expand_other_math_functions() {
        let values = [Value::Byte(1), Value::Byte(2), Value::Byte(3)];
        for function in [
            MathFunction::Round,
            MathFunction::Abs,
            MathFunction::QbColor,
        ] {
            assert_eq!(
                function.apply(&values),
                Err(EvalError::UnsupportedExpression)
            );
        }
        for arguments in [&values[..0], &values[..1], &values[..2]] {
            assert_eq!(
                MathFunction::Rgb.apply(arguments),
                Err(EvalError::UnsupportedExpression)
            );
        }

        assert_eq!(MathFunction::resolve("rGb"), Some(MathFunction::Rgb));
        assert_eq!(
            MathFunction::resolve("qbCOLOR"),
            Some(MathFunction::QbColor)
        );
        assert_eq!(MathFunction::resolve("RGB$"), None);
    }

    proptest! {
        #![proptest_config(crate::test_support::config())]

        #[test]
        fn packed_channels_match_base_256_arithmetic(red in 0_u16..=1024, green in 0_u16..=1024, blue in 0_u16..=1024) {
            let expected = i32::from(red.min(255))
                + 256 * i32::from(green.min(255))
                + 65536 * i32::from(blue.min(255));
            let arguments = [red, green, blue].map(|channel| Value::Int32(i32::from(channel)));

            prop_assert_eq!(MathFunction::Rgb.apply(&arguments), Ok(Value::Int32(expected)));
        }
    }
}

use super::*;
use crate::functions::text::apply;
use proptest::prelude::*;
use tablecraft_core::values::{
    Currency, Decimal, Double, ExtendedDateTime, LegacyDateTime, Single,
};

struct Output {
    text: String,
    steps: usize,
    bytes: usize,
}
impl TextOutput for Output {
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
fn render(
    function: TextFunction,
    value: Value,
    steps: usize,
    bytes: usize,
) -> Result<Value, EvalError> {
    let args = [value];
    match apply(function, &args, || Ok(()))? {
        TextResult::Null => Ok(Value::Null),
        TextResult::NumericFormat(format) => {
            let mut output = Output {
                text: String::new(),
                steps,
                bytes,
            };
            format.write(&mut output)?;
            Ok(Value::Text(output.text))
        }
        _ => unreachable!(),
    }
}
fn text(text: &str) -> Value {
    Value::Text(text.into())
}
fn call(function: TextFunction, value: Value) -> Result<Value, EvalError> {
    render(function, value, 1000, 1000)
}
#[test]
fn radix_examples_boundaries_and_half_even_conversion() {
    for (value, hex, oct) in [
        (0, "0", "0"),
        (5, "5", "5"),
        (8, "8", "10"),
        (10, "A", "12"),
        (459, "1CB", "713"),
        (-1, "FFFF", "177777"),
        (-32767, "8001", "100001"),
        (-32768, "FFFF8000", "37777700000"),
        (-32769, "FFFF7FFF", "37777677777"),
        (i32::MIN as i64, "80000000", "20000000000"),
        (i64::MIN, "8000000000000000", "1000000000000000000000"),
        (i64::MAX, "7FFFFFFFFFFFFFFF", "777777777777777777777"),
    ] {
        assert_eq!(
            call(TextFunction::Hex, Value::Int64(value)),
            Ok(text(hex)),
            "{value}"
        );
        assert_eq!(
            call(TextFunction::Oct, Value::Int64(value)),
            Ok(text(oct)),
            "{value}"
        );
    }
    for (value, expected) in [(1.5, "2"), (2.5, "2"), (-1.5, "FFFE"), (-2.5, "FFFE")] {
        assert_eq!(
            call(TextFunction::Hex, Value::Double(Double::from_float(value))),
            Ok(text(expected))
        );
    }
    assert_eq!(
        call(TextFunction::Hex, Value::Boolean(true)),
        Ok(text("FFFF"))
    );
    assert_eq!(
        call(
            TextFunction::Hex,
            Value::Currency(Currency::from_scaled(25_000))
        ),
        Ok(text("2"))
    );
    assert_eq!(
        call(
            TextFunction::Oct,
            Value::Decimal(Decimal::new(35, 1).unwrap())
        ),
        Ok(text("4"))
    );
}
#[test]
fn str_preserves_exact_fixed_carriers_and_reserves_the_sign() {
    for (value, expected) in [
        (Value::Int64(i64::MIN), "-9223372036854775808"),
        (Value::Int64(i64::MAX), " 9223372036854775807"),
        (Value::Boolean(true), "-1"),
        (Value::Boolean(false), " 0"),
        (
            Value::Currency(Currency::from_scaled(i64::MIN)),
            "-922337203685477.5808",
        ),
        (Value::Currency(Currency::from_scaled(1)), " 0.0001"),
        (Value::Currency(Currency::from_scaled(12_000)), " 1.2"),
        (Value::Decimal(Decimal::new(0, 28).unwrap()), " 0"),
        (
            Value::Decimal(Decimal::new(Decimal::MAX_COEFFICIENT, 0).unwrap()),
            " 79228162514264337593543950335",
        ),
        (
            Value::Decimal(Decimal::new(1, 28).unwrap()),
            " 0.0000000000000000000000000001",
        ),
    ] {
        assert_eq!(call(TextFunction::Str, value), Ok(text(expected)));
    }
}
#[test]
fn str_finite_floats_round_significant_digits_with_explicit_notation() {
    for (value, expected) in [
        (459.0, " 459"),
        (-459.65, "-459.65"),
        (459.001, " 459.001"),
        (0.0, " 0"),
        (-0.0, " 0"),
        (0.5, " 0.5"),
        (1e-20, " 0.00000000000000000001"),
        (1e15, " 1E+15"),
        (1234567890123456.0, " 1.23456789012346E+15"),
        (1.2345678901234567, " 1.23456789012346"),
    ] {
        assert_eq!(
            call(TextFunction::Str, Value::Double(Double::from_float(value))),
            Ok(text(expected)),
            "{value}"
        );
    }
    assert_eq!(
        call(TextFunction::Str, Value::Single(Single::from_float(0.1))),
        Ok(text(" 0.1"))
    );
    assert_eq!(
        call(
            TextFunction::Str,
            Value::Single(Single::from_float(12345678.0))
        ),
        Ok(text(" 1.234568E+7"))
    );
    for value in [f64::MAX, f64::MIN_POSITIVE, f64::from_bits(1)] {
        let Value::Text(result) =
            call(TextFunction::Str, Value::Double(Double::from_float(value))).unwrap()
        else {
            panic!()
        };
        assert!(result.len() < 400);
        assert!(!result.contains("inf") && !result.contains("NaN"));
    }
}
#[test]
fn null_type_nonfinite_and_resource_failures_are_explicit() {
    for function in [TextFunction::Hex, TextFunction::Oct, TextFunction::Str] {
        assert_eq!(call(function, Value::Null), Ok(Value::Null));
        assert_eq!(call(function, text("123")), Err(EvalError::TypeMismatch));
        assert!(call(function, Value::Double(Double::from_float(f64::NAN))).is_err());
        assert!(call(function, Value::Double(Double::from_float(f64::INFINITY))).is_err());
        assert_eq!(
            render(function, Value::Int16(123), 0, 10),
            Err(EvalError::Cancelled)
        );
        assert_eq!(
            render(function, Value::Int16(123), 100, 1),
            Err(EvalError::TextLimit)
        );
    }
    assert_eq!(
        call(
            TextFunction::Hex,
            Value::Double(Double::from_float(2147483648.0))
        ),
        Err(EvalError::Overflow)
    );
    assert_eq!(
        call(TextFunction::Hex, Value::Int64(2147483648)),
        Ok(text("80000000"))
    );
}
proptest! {
    #![proptest_config(crate::test_support::config())]
    #[test]
    fn positive_longlong_formats_match_standard_integer_model(value in 0..=i64::MAX) {
        prop_assert_eq!(call(TextFunction::Hex,Value::Int64(value)),Ok(text(&format!("{value:X}"))));
        prop_assert_eq!(call(TextFunction::Oct,Value::Int64(value)),Ok(text(&format!("{value:o}"))));
        prop_assert_eq!(call(TextFunction::Str,Value::Int64(value)),Ok(text(&format!(" {value}"))));
    }
    #[test]
    fn decimal_str_does_not_lose_fixed_precision(coefficient in any::<i64>(), scale in 0..=28u8) {
        let value = Decimal::new(i128::from(coefficient),scale).unwrap();
        let Value::Text(result)=call(TextFunction::Str,Value::Decimal(value)).unwrap() else { unreachable!() };
        let unsigned=result.trim_start().trim_start_matches('-');
        let (whole,frac)=unsigned.split_once('.').unwrap_or((unsigned,""));
        let digits = format!("{whole}{frac}").parse::<i128>().unwrap();
        let signed = if coefficient<0 {-digits} else {digits};
        prop_assert_eq!(signed*10_i128.pow(u32::from(scale-frac.len() as u8)), i128::from(coefficient));
    }
}

#[test]
fn radix_dates_share_long_rounding_and_keep_extended_dates_separate() {
    for (serial, hex, octal) in [
        (2.5, "2", "2"),
        (3.5, "4", "4"),
        (-1.5, "FFFE", "177776"),
        (-32768.5, "FFFF8000", "37777700000"),
    ] {
        let date = Value::DateTime(LegacyDateTime::from_ole_days(serial).unwrap());
        assert_eq!(call(TextFunction::Hex, date.clone()), Ok(text(hex)));
        assert_eq!(call(TextFunction::Oct, date.clone()), Ok(text(octal)));
        assert_eq!(call(TextFunction::Str, date), Err(EvalError::TypeMismatch));
    }
    for function in [TextFunction::Hex, TextFunction::Oct] {
        let value =
            Value::DateTimeExtended(ExtendedDateTime::new(2026, 10, 23, 0, 0, 0, 0).unwrap());
        assert_eq!(call(function, value), Err(EvalError::TypeMismatch));
    }
}

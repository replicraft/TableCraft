use super::*;
use crate::{
    functions::conversions::text::NumberFormat,
    functions::text::{TextFunction, apply_with_context, output::TextOutput},
};
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
    value: Value,
    number: Option<NumberFormat>,
    date: Option<DateTextFormat>,
    steps: usize,
    bytes: usize,
) -> Result<Value, EvalError> {
    let args = [value];
    let result = apply_with_context(
        TextFunction::CStr,
        &args,
        TextContext {
            number_format: number,
            date_format: date,
            ..Default::default()
        },
        || Ok(()),
    )?;
    let mut output = Output {
        text: String::new(),
        steps,
        bytes,
    };
    match result {
        TextResult::Text(text) => output.append(text)?,
        TextResult::NumericFormat(format) => format.write(&mut output)?,
        TextResult::DateFormat(format) => format.write(&mut output)?,
        _ => unreachable!(),
    }
    Ok(Value::Text(output.text))
}
fn text(text: &str) -> Value {
    Value::Text(text.into())
}
fn call(
    value: Value,
    number: Option<NumberFormat>,
    date: Option<DateTextFormat>,
) -> Result<Value, EvalError> {
    render(value, number, date, 1000, 1000)
}
#[test]
fn string_boolean_null_and_exact_numeric_conversion_are_distinct() {
    assert_eq!(call(text("1.23\0é"), None, None), Ok(text("1.23\0é")));
    assert_eq!(call(Value::Boolean(true), None, None), Ok(text("True")));
    assert_eq!(call(Value::Boolean(false), None, None), Ok(text("False")));
    assert_eq!(
        call(Value::Null, None, None),
        Err(EvalError::InvalidNullConversion)
    );
    assert_eq!(
        call(Value::Binary(vec![]), None, None),
        Err(EvalError::TypeMismatch)
    );
    for (value, expected) in [
        (Value::Int64(i64::MIN), "-9223372036854775808"),
        (Value::Int64(i64::MAX), "9223372036854775807"),
        (Value::Currency(Currency::from_scaled(12_000)), "1.2"),
        (
            Value::Currency(Currency::from_scaled(i64::MIN)),
            "-922337203685477.5808",
        ),
        (
            Value::Decimal(Decimal::new(Decimal::MAX_COEFFICIENT, 0).unwrap()),
            "79228162514264337593543950335",
        ),
        (
            Value::Decimal(Decimal::new(1, 28).unwrap()),
            "0.0000000000000000000000000001",
        ),
    ] {
        assert_eq!(
            call(value, Some(NumberFormat::Invariant), None),
            Ok(text(expected))
        );
    }
    assert_eq!(
        call(Value::Int16(1), None, None),
        Err(EvalError::NumberFormatRequired)
    );
}
#[test]
fn numeric_profiles_change_only_numeric_decimal_output_and_preserve_text() {
    for (value, expected) in [
        (Value::Double(Double::from_float(0.5)), "0,5"),
        (Value::Double(Double::from_float(-1.25)), "-1,25"),
        (
            Value::Double(Double::from_float(1.234567890123456e16)),
            "1,23456789012346E+16",
        ),
        (Value::Single(Single::from_float(0.1)), "0,1"),
        (Value::Double(Double::from_float(-0.0)), "0"),
        (Value::Currency(Currency::from_scaled(12_000)), "1,2"),
    ] {
        assert_eq!(
            call(value, Some(NumberFormat::DeDe), None),
            Ok(text(expected))
        );
    }
    assert_eq!(
        call(text("1.23"), Some(NumberFormat::DeDe), None),
        Ok(text("1.23"))
    );
    assert!(
        call(
            Value::Double(Double::from_float(f64::INFINITY)),
            Some(NumberFormat::Invariant),
            None
        )
        .is_err()
    );
}
#[test]
fn date_profiles_cover_date_time_epoch_carry_and_checked_ranges() {
    let date = Value::DateTimeExtended(ExtendedDateTime::new(2026, 10, 20, 13, 4, 5, 0).unwrap());
    for (format, expected) in [
        (DateTextFormat::Iso, "2026-10-20 13:04:05"),
        (DateTextFormat::EnUs, "10/20/2026 1:04:05 PM"),
        (DateTextFormat::DeDe, "20.10.2026 13:04:05"),
    ] {
        assert_eq!(call(date.clone(), None, Some(format)), Ok(text(expected)));
    }
    assert_eq!(call(date, None, None), Err(EvalError::DateFormatRequired));
    let epoch = Value::DateTime(LegacyDateTime::from_ole_days(0.0).unwrap());
    assert_eq!(
        call(epoch, None, Some(DateTextFormat::EnUs)),
        Ok(text("12:00:00 AM"))
    );
    let midnight =
        Value::DateTimeExtended(ExtendedDateTime::new(2026, 10, 20, 0, 0, 0, 0).unwrap());
    assert_eq!(
        call(midnight, None, Some(DateTextFormat::Iso)),
        Ok(text("2026-10-20"))
    );
    let carry = Value::DateTimeExtended(
        ExtendedDateTime::new(2026, 12, 31, 23, 59, 59, 5_000_000).unwrap(),
    );
    assert_eq!(
        call(carry, None, Some(DateTextFormat::Iso)),
        Ok(text("2027-01-01"))
    );
    let overflow = Value::DateTimeExtended(
        ExtendedDateTime::new(9999, 12, 31, 23, 59, 59, 9_999_999).unwrap(),
    );
    assert_eq!(
        call(overflow, None, Some(DateTextFormat::Iso)),
        Err(EvalError::Overflow)
    );
    assert_eq!(
        call(
            Value::DateTimeExtended(ExtendedDateTime::new(1, 1, 1, 0, 0, 0, 0).unwrap()),
            None,
            Some(DateTextFormat::Iso)
        ),
        Ok(text("0001-01-01"))
    );
}
#[test]
fn cstr_output_obeys_limits_and_cancellation() {
    assert_eq!(
        render(
            Value::Int64(1234),
            Some(NumberFormat::Invariant),
            None,
            100,
            3
        ),
        Err(EvalError::TextLimit)
    );
    assert_eq!(
        render(
            Value::Int64(1234),
            Some(NumberFormat::Invariant),
            None,
            0,
            100
        ),
        Err(EvalError::Cancelled)
    );
    assert_eq!(
        render(
            Value::DateTime(LegacyDateTime::from_ole_days(1.0).unwrap()),
            None,
            Some(DateTextFormat::Iso),
            100,
            3
        ),
        Err(EvalError::TextLimit)
    );
}
proptest! {
    #![proptest_config(crate::test_support::config())]
    #[test]
    fn full_integer_conversion_matches_exact_integer_model(value in any::<i64>()){
        prop_assert_eq!(call(Value::Int64(value),Some(NumberFormat::Invariant),None),Ok(text(&value.to_string())));
    }
}

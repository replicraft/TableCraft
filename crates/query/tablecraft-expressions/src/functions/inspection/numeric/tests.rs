use super::*;
use crate::{
    functions::text::output::TextOutput,
    functions::{Builtin, inspection::InspectionFunction},
};
use proptest::prelude::*;
use tablecraft_core::values::{
    Currency, Decimal, Double, ExtendedDateTime, Guid, LegacyDateTime, Single,
};

struct Buffer {
    text: String,
    steps: usize,
    bytes: usize,
    append_error: Option<EvalError>,
}
impl TextOutput for Buffer {
    fn step(&mut self) -> Result<(), EvalError> {
        self.steps = self.steps.checked_sub(1).ok_or(EvalError::Cancelled)?;
        Ok(())
    }
    fn append(&mut self, text: &str) -> Result<(), EvalError> {
        self.step()?;
        if let Some(error) = self.append_error {
            return Err(error);
        }
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
fn buffer() -> Buffer {
    Buffer {
        text: String::new(),
        steps: 100_000,
        bytes: 65_536,
        append_error: None,
    }
}
fn call(value: Value, format: Option<NumberFormat>) -> Result<Value, EvalError> {
    InspectionFunction::IsNumeric.apply(&value, format, &mut buffer())
}
fn text(text: &str) -> Value {
    Value::Text(text.into())
}
#[test]
fn numeric_boolean_carriers_are_recognized_by_type_including_ieee_bits() {
    for value in [
        Value::Boolean(false),
        Value::Boolean(true),
        Value::Byte(0),
        Value::Int16(i16::MIN),
        Value::Int32(i32::MAX),
        Value::Int64(i64::MIN),
        Value::Int64(i64::MAX),
        Value::Single(Single::from_float(0.0)),
        Value::Single(Single::from_float(f32::NAN)),
        Value::Double(Double::from_float(f64::INFINITY)),
        Value::Currency(Currency::from_scaled(i64::MIN)),
        Value::Decimal(Decimal::new(1, 28).unwrap()),
    ] {
        assert_eq!(call(value, None), Ok(Value::Boolean(true)));
    }
    for value in [
        Value::Null,
        Value::Binary(vec![49, 50]),
        Value::Guid(Guid::from_bytes([0; 16])),
        Value::DateTime(LegacyDateTime::from_ole_days(0.0).unwrap()),
        Value::DateTimeExtended(ExtendedDateTime::new(2026, 10, 20, 0, 0, 0, 0).unwrap()),
    ] {
        assert_eq!(call(value, None), Ok(Value::Boolean(false)));
    }
    assert_eq!(
        Builtin::resolve("iSnUmErIc"),
        Some(Builtin::Inspect(InspectionFunction::IsNumeric))
    );
    assert_eq!(Builtin::resolve("IsNumeric$"), None);
}
#[test]
fn text_requires_complete_valid_finite_numeric_input_under_explicit_profiles() {
    for (format, value) in [
        (NumberFormat::Invariant, "53"),
        (NumberFormat::Invariant, "459.95"),
        (NumberFormat::EnUs, "($1,234.50)"),
        (NumberFormat::DeDe, "1.234,50 €"),
        (NumberFormat::Invariant, "1e-999999"),
        (NumberFormat::Invariant, "-0"),
    ] {
        assert_eq!(
            call(text(value), Some(format)),
            Ok(Value::Boolean(true)),
            "{value}"
        );
    }
    for value in [
        "45 Help", "1e999999", "True", "#TRUE#", "NaN", "Infinity", "1,23.0", "&HFF", "1%", "",
        "12 34",
    ] {
        assert_eq!(
            call(text(value), Some(NumberFormat::EnUs)),
            Ok(Value::Boolean(false)),
            "{value}"
        );
    }
    assert_eq!(call(text("53"), None), Err(EvalError::NumberFormatRequired));
    assert_eq!(call(text(""), None), Err(EvalError::NumberFormatRequired));
}
#[test]
fn operational_failures_are_not_nonnumeric_classifications() {
    for error in [
        EvalError::Cancelled,
        EvalError::Allocation,
        EvalError::TextLimit,
        EvalError::TypeMismatch,
        EvalError::Overflow,
        EvalError::Domain,
    ] {
        let mut buffer = buffer();
        buffer.append_error = Some(error);
        assert_eq!(
            InspectionFunction::IsNumeric.apply(
                &text("123"),
                Some(NumberFormat::Invariant),
                &mut buffer
            ),
            Err(error)
        );
    }
    let mut output = buffer();
    output.steps = 2;
    assert_eq!(
        InspectionFunction::IsNumeric.apply(
            &text("123"),
            Some(NumberFormat::Invariant),
            &mut output
        ),
        Err(EvalError::Cancelled)
    );
    let mut output = buffer();
    output.bytes = 2;
    assert_eq!(
        InspectionFunction::IsNumeric.apply(
            &text("123"),
            Some(NumberFormat::Invariant),
            &mut output
        ),
        Err(EvalError::TextLimit)
    );
}
proptest! {
    #![proptest_config(crate::test_support::config())]
    #[test]
    fn full_integer_text_is_numeric_and_trailing_junk_is_not(value in any::<i64>()){
        prop_assert_eq!(call(text(&value.to_string()),Some(NumberFormat::Invariant)),Ok(Value::Boolean(true)));
        prop_assert_eq!(call(text(&format!("{value}x")),Some(NumberFormat::Invariant)),Ok(Value::Boolean(false)));
    }
}

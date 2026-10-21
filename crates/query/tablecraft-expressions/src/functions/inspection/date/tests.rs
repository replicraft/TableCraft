use super::*;
use crate::functions::{
    Builtin,
    dates::parsing::{DateTextFormat, ReferenceYear},
    inspection::InspectionFunction,
    text::output::TextOutput,
};
use proptest::prelude::*;
use tablecraft_core::values::{Double, ExtendedDateTime, Guid, LegacyDateTime};

#[derive(Default)]
struct Buffer {
    remaining: Option<usize>,
    failure: Option<EvalError>,
}
impl TextOutput for Buffer {
    fn step(&mut self) -> Result<(), EvalError> {
        if let Some(remaining) = &mut self.remaining {
            *remaining = remaining
                .checked_sub(1)
                .ok_or(self.failure.unwrap_or(EvalError::Cancelled))?;
        }
        Ok(())
    }
    fn append(&mut self, _: &str) -> Result<(), EvalError> {
        panic!("calendar recognition must not allocate normalization text")
    }
}
impl NumericBuffer for Buffer {
    fn text(&self) -> &str {
        ""
    }
}
fn context() -> DateParseContext {
    DateParseContext {
        date_format: Some(DateTextFormat::EnUs),
        ..Default::default()
    }
}
fn call(value: Value, context: DateParseContext) -> Result<Value, EvalError> {
    InspectionFunction::IsDate.apply_with_context(&value, context, &mut Buffer::default())
}
fn text(value: &str) -> Value {
    Value::Text(value.into())
}
#[test]
fn materialized_types_ranges_and_identity_are_explicit() {
    for value in [
        Value::Int16(0),
        Value::Int32(-1),
        Value::Double(Double::from_float(0.25)),
        Value::DateTime(LegacyDateTime::from_ole_days(-0.25).unwrap()),
        Value::DateTimeExtended(ExtendedDateTime::new(2026, 10, 20, 0, 0, 0, 0).unwrap()),
    ] {
        assert_eq!(
            call(value, DateParseContext::default()),
            Ok(Value::Boolean(true))
        );
    }
    for value in [
        Value::Null,
        Value::Boolean(true),
        Value::Boolean(false),
        Value::Binary(vec![]),
        Value::Guid(Guid::from_bytes([0; 16])),
        Value::Int64(i64::MAX),
        Value::Double(Double::from_float(f64::NAN)),
        Value::Double(Double::from_float(f64::INFINITY)),
        Value::DateTimeExtended(ExtendedDateTime::new(1, 1, 1, 0, 0, 0, 0).unwrap()),
    ] {
        assert_eq!(
            call(value, DateParseContext::default()),
            Ok(Value::Boolean(false))
        );
    }
    assert_eq!(
        Builtin::resolve("iSdAtE"),
        Some(Builtin::Inspect(InspectionFunction::IsDate))
    );
    assert_eq!(Builtin::resolve("IsDate$"), None);
}
#[test]
fn whole_calendar_text_recognition_retains_policy_requirements() {
    for value in [
        "04/28/2014",
        "April 28, 2014",
        "4:28:14",
        "4:28 PM",
        "04.28.14",
        "2024-02-29",
    ] {
        assert_eq!(
            call(text(value), context()),
            Ok(Value::Boolean(true)),
            "{value}"
        );
    }
    for value in [
        "13/32/2014",
        "04.28.2014",
        "2023-02-29",
        "2024-02-29 25:00",
        "",
        "123",
        "1.25",
        "True",
        "2024-02-29 junk",
    ] {
        assert_eq!(
            call(text(value), context()),
            Ok(Value::Boolean(false)),
            "{value}"
        );
    }
    assert_eq!(
        call(text("2024-02-29"), DateParseContext::default()),
        Err(EvalError::DateFormatRequired)
    );
    assert_eq!(
        call(text("Feb 29"), context()),
        Err(EvalError::ReferenceYearRequired)
    );
    for (year, expected) in [(2024, true), (2023, false)] {
        assert_eq!(
            call(
                text("Feb 29"),
                DateParseContext {
                    reference_year: Some(ReferenceYear::new(year).unwrap()),
                    ..context()
                }
            ),
            Ok(Value::Boolean(expected))
        );
    }
}
#[test]
fn host_errors_cannot_be_reclassified_as_invalid_dates() {
    for failure in [
        EvalError::Cancelled,
        EvalError::Allocation,
        EvalError::TextLimit,
        EvalError::TypeMismatch,
        EvalError::Overflow,
        EvalError::NonFinite,
        EvalError::Domain,
    ] {
        let mut buffer = Buffer {
            remaining: Some(3),
            failure: Some(failure),
        };
        assert_eq!(
            InspectionFunction::IsDate.apply_with_context(
                &text("04/28/2014"),
                context(),
                &mut buffer
            ),
            Err(failure)
        );
    }
}
proptest! {
    #![proptest_config(crate::test_support::config())]
    #[test]
    fn generated_gregorian_dates_and_trailing_junk(year in 100_i16..=9999, month in 1_i8..=12, day in 1_i8..=31) {
        let spelling = format!("{year:04}-{month:02}-{day:02}");
        let expected = jiff::civil::Date::new(year, month, day).is_ok();
        prop_assert_eq!(call(text(&spelling), context()), Ok(Value::Boolean(expected)));
        prop_assert_eq!(call(text(&format!("{spelling}x")), context()), Ok(Value::Boolean(false)));
    }
}

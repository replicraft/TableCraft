use super::*;
use crate::functions::text::output::TextOutput;
use proptest::prelude::*;
use tablecraft_core::values::{Double, ExtendedDateTime};
struct Buffer {
    text: String,
    steps: usize,
    bytes: usize,
    error: Option<EvalError>,
}
impl TextOutput for Buffer {
    fn step(&mut self) -> Result<(), EvalError> {
        self.steps = self.steps.checked_sub(1).ok_or(EvalError::Cancelled)?;
        Ok(())
    }
    fn append(&mut self, text: &str) -> Result<(), EvalError> {
        self.step()?;
        if let Some(error) = self.error {
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
        error: None,
    }
}
fn call(
    function: DateConversion,
    value: Value,
    context: DateParseContext,
) -> Result<Value, EvalError> {
    function.apply(&value, context, &mut buffer())
}
fn text(value: &str) -> Value {
    Value::Text(value.into())
}
fn date(days: f64) -> Value {
    Value::DateTime(LegacyDateTime::from_ole_days(days).unwrap())
}
fn context(format: DateTextFormat) -> DateParseContext {
    DateParseContext {
        date_format: Some(format),
        ..Default::default()
    }
}
fn calendar(value: Value) -> jiff::civil::DateTime {
    let Value::DateTime(value) = value else {
        panic!()
    };
    value.calendar_rounded_to_nanos().unwrap()
}
#[test]
fn numeric_date_values_keep_bits_aliases_and_unrounded_extraction() {
    for days in [
        0.0,
        -0.0,
        0.25,
        -0.25,
        -1.25,
        45000.9999999,
        f64::from(LegacyDateTime::MAX_DAY) + 0.9999999995,
    ] {
        let value = date(days);
        assert_eq!(
            call(DateConversion::CDate, value.clone(), Default::default()),
            Ok(value.clone())
        );
        assert_eq!(
            call(
                DateConversion::CDate,
                Value::Double(Double::from_float(days)),
                Default::default()
            ),
            Ok(value)
        );
        assert_eq!(
            call(DateConversion::DateValue, date(days), Default::default()),
            Ok(date(if days.trunc() == 0.0 {
                0.0
            } else {
                days.trunc()
            }))
        );
        assert_eq!(
            call(DateConversion::TimeValue, date(days), Default::default()),
            Ok(date((days - days.trunc()).abs()))
        );
    }
    assert_eq!(
        call(
            DateConversion::CDate,
            Value::Boolean(true),
            Default::default()
        ),
        Ok(date(-1.0))
    );
    assert_eq!(
        call(DateConversion::CDate, Value::Null, Default::default()),
        Err(EvalError::InvalidNullConversion)
    );
    assert_eq!(
        call(DateConversion::DateValue, Value::Null, Default::default()),
        Ok(Value::Null)
    );
    assert_eq!(
        call(DateConversion::TimeValue, Value::Null, Default::default()),
        Ok(Value::Null)
    );
    assert_eq!(
        call(
            DateConversion::CDate,
            Value::Double(Double::from_float(f64::INFINITY)),
            Default::default()
        ),
        Err(EvalError::NonFinite)
    );
    assert_eq!(
        call(
            DateConversion::CDate,
            Value::Int64(i64::MAX),
            Default::default()
        ),
        Err(EvalError::Overflow)
    );
}
#[test]
fn regional_dates_month_names_and_12_24_hour_time_validate_completely() {
    for (format, value) in [
        (DateTextFormat::Iso, "1969-02-12 16:35:17"),
        (DateTextFormat::EnUs, "2/12/1969 4:35:17 PM"),
        (DateTextFormat::DeDe, "12.2.1969 16:35:17"),
        (DateTextFormat::EnUs, "February 12, 1969 4:35:17PM"),
        (
            DateTextFormat::EnUs,
            "February 12,\u{00a0}1969\u{2009}4:35:17PM",
        ),
        (DateTextFormat::Iso, "12-Feb-69 16:35:17"),
        (DateTextFormat::DeDe, "1969-02-12 16:35:17"),
    ] {
        let result = call(DateConversion::CDate, text(value), context(format)).unwrap();
        // Independent Automation serial bits: day 25246 plus 59717/86400.
        assert_eq!(
            result,
            Value::DateTime(LegacyDateTime::from_bits(0x40d8_a7ac_3c1c_d2de).unwrap()),
            "{value}"
        );
        assert_eq!(
            calendar(call(DateConversion::DateValue, text(value), context(format)).unwrap())
                .to_string(),
            "1969-02-12T00:00:00"
        );
        assert_eq!(
            calendar(call(DateConversion::TimeValue, text(value), context(format)).unwrap())
                .to_string(),
            "1899-12-30T16:35:17"
        );
    }
    for (value, hour, minute) in [
        ("2:24PM", 14, 24),
        ("14:24", 14, 24),
        ("12 AM", 0, 0),
        ("12PM", 12, 0),
    ] {
        let result = calendar(
            call(
                DateConversion::TimeValue,
                text(value),
                context(DateTextFormat::EnUs),
            )
            .unwrap(),
        );
        assert_eq!((result.hour(), result.minute()), (hour, minute));
    }
    assert_eq!(
        calendar(
            call(
                DateConversion::DateValue,
                text("9999-12-31 23:59:59.999999999"),
                context(DateTextFormat::Iso)
            )
            .unwrap()
        )
        .to_string(),
        "9999-12-31T00:00:00"
    );
    for value in [
        "2026-02-30 12:00",
        "2026-01-01 89:98",
        "2026-01-01 23:59:60",
        "2026-01-01 00:00Z",
        "2026-01-01T00:00+01:00",
        "2026-01-01 trailing",
        "2026-01-01 0:00 PM",
    ] {
        for function in [DateConversion::DateValue, DateConversion::TimeValue] {
            assert!(
                call(function, text(value), context(DateTextFormat::Iso)).is_err(),
                "{value}"
            );
        }
    }
}
#[test]
fn explicit_reference_year_and_year_window_replace_clock_dependencies() {
    let mut ctx = context(DateTextFormat::EnUs);
    assert_eq!(
        call(DateConversion::DateValue, text("Feb 29"), ctx),
        Err(EvalError::ReferenceYearRequired)
    );
    ctx.reference_year = Some(ReferenceYear::new(2024).unwrap());
    assert_eq!(
        calendar(call(DateConversion::DateValue, text("Feb 29"), ctx).unwrap())
            .date()
            .to_string(),
        "2024-02-29"
    );
    ctx.reference_year = Some(ReferenceYear::new(2023).unwrap());
    assert_eq!(
        call(DateConversion::DateValue, text("2/29"), ctx),
        Err(EvalError::TypeMismatch)
    );
    ctx.year_window = super::super::TwoDigitYearWindow::new(2049).unwrap();
    assert_eq!(
        calendar(call(DateConversion::DateValue, text("1/2/49"), ctx).unwrap())
            .date()
            .year(),
        2049
    );
    assert!(ReferenceYear::new(99).is_err());
    assert!(ReferenceYear::new(10000).is_err());
}
#[test]
fn extended_dates_project_before_legacy_narrowing_and_keep_subseconds() {
    let value = Value::DateTimeExtended(
        ExtendedDateTime::new(2026, 10, 20, 23, 59, 59, 9_999_999).unwrap(),
    );
    assert_eq!(
        calendar(call(DateConversion::DateValue, value, Default::default()).unwrap()).to_string(),
        "2026-10-20T00:00:00"
    );
    let early = Value::DateTimeExtended(ExtendedDateTime::new(1, 1, 1, 6, 0, 0, 0).unwrap());
    assert_eq!(
        call(DateConversion::TimeValue, early.clone(), Default::default()),
        Ok(date(0.25))
    );
    assert_eq!(
        call(DateConversion::CDate, early, Default::default()),
        Err(EvalError::Overflow)
    );
}
#[test]
fn numeric_text_fallback_is_explicit_and_operational_failures_propagate() {
    let mut ctx = context(DateTextFormat::Iso);
    ctx.number_format = Some(crate::functions::conversions::text::NumberFormat::Invariant);
    assert_eq!(
        call(DateConversion::CDate, text("-1.25"), ctx),
        Ok(date(-1.25))
    );
    assert_eq!(
        call(DateConversion::CDate, text("1e999"), ctx),
        Err(EvalError::TypeMismatch)
    );
    assert_eq!(
        call(
            DateConversion::CDate,
            text("2026-01-01"),
            Default::default()
        ),
        Err(EvalError::DateFormatRequired)
    );
    let mut output = buffer();
    output.steps = 3;
    assert_eq!(
        DateConversion::CDate.apply(&text("2026-01-01"), ctx, &mut output),
        Err(EvalError::Cancelled)
    );
    for error in [
        EvalError::Allocation,
        EvalError::Overflow,
        EvalError::TextLimit,
    ] {
        let mut output = buffer();
        output.error = Some(error);
        assert_eq!(
            DateConversion::CDate.apply(&text("123"), ctx, &mut output),
            Err(error)
        );
    }
}
proptest! {
    #![proptest_config(crate::test_support::config())]
    #[test]
    fn whole_serial_days_project_without_a_calendar_round_trip(days in -657_434i32..=2_958_465i32){
        prop_assert_eq!(call(DateConversion::DateValue,date(f64::from(days)),Default::default()),Ok(date(f64::from(days))));
        prop_assert_eq!(call(DateConversion::TimeValue,date(f64::from(days)),Default::default()),Ok(date(0.0)));
    }
}

use super::*;
use crate::functions::{
    Builtin,
    dates::{
        FirstDayOfWeek,
        parsing::{DateTextFormat, ReferenceYear},
    },
    text::output::TextOutput,
};
use proptest::prelude::*;
use tablecraft_core::values::{Double, ExtendedDateTime, LegacyDateTime};
#[derive(Default)]
struct Buffer {
    text: String,
    failure: Option<EvalError>,
}
impl TextOutput for Buffer {
    fn step(&mut self) -> Result<(), EvalError> {
        self.failure.map_or(Ok(()), Err)
    }
    fn append(&mut self, text: &str) -> Result<(), EvalError> {
        self.step()?;
        self.text.push_str(text);
        Ok(())
    }
}
impl NumericBuffer for Buffer {
    fn text(&self) -> &str {
        &self.text
    }
}
fn call(
    value: &Value,
    first: Option<&Value>,
    week: WeekContext,
    date: DateParseContext,
) -> Result<Value, EvalError> {
    weekday(value, first, week, date, &mut Buffer::default())
}
fn legacy(value: &str) -> Value {
    Value::DateTime(LegacyDateTime::from_calendar(value.parse().unwrap()).unwrap())
}
#[test]
fn weekday_defaults_overrides_system_and_null_are_explicit() {
    let date = legacy("1969-02-12T00:00:00");
    let context = WeekContext {
        first_day: Some(FirstDayOfWeek::new(2).unwrap()),
        ..Default::default()
    };
    assert_eq!(
        call(&date, None, context, DateParseContext::default()),
        Ok(Value::Int16(4))
    );
    assert_eq!(
        call(
            &date,
            Some(&Value::Int16(0)),
            context,
            DateParseContext::default()
        ),
        Ok(Value::Int16(3))
    );
    assert_eq!(
        call(
            &date,
            Some(&Value::Int16(1)),
            context,
            DateParseContext::default()
        ),
        Ok(Value::Int16(4))
    );
    assert_eq!(
        call(
            &Value::Null,
            None,
            WeekContext::default(),
            DateParseContext::default()
        ),
        Ok(Value::Null)
    );
    assert_eq!(
        call(
            &date,
            Some(&Value::Int16(0)),
            WeekContext::default(),
            DateParseContext::default()
        ),
        Err(EvalError::CalendarPolicyRequired)
    );
    assert_eq!(
        call(
            &Value::Null,
            Some(&Value::Int16(8)),
            context,
            DateParseContext::default()
        ),
        Err(EvalError::Domain)
    );
    assert_eq!(
        call(
            &date,
            Some(&Value::Null),
            context,
            DateParseContext::default()
        ),
        Err(EvalError::TypeMismatch)
    );
    assert_eq!(Builtin::resolve("wEeKdAy"), Some(Builtin::Weekday));
    assert_eq!(Builtin::resolve("Weekday$"), None);
}
#[test]
fn numeric_text_and_extended_inputs_follow_the_shared_date_profile() {
    let context = DateParseContext {
        date_format: Some(DateTextFormat::Iso),
        reference_year: Some(ReferenceYear::new(2024).unwrap()),
        number_format: Some(crate::functions::conversions::text::NumberFormat::Invariant),
        ..Default::default()
    };
    for (value, expected) in [
        (Value::Int16(0), 7),
        (Value::Int16(-1), 6),
        (Value::Boolean(true), 6),
        (Value::Double(Double::from_float(-1.25)), 6),
        (Value::Text("-1.25".into()), 6),
        (Value::Text("Feb 29".into()), 5),
        (
            Value::DateTimeExtended(ExtendedDateTime::new(1, 1, 1, 0, 0, 0, 0).unwrap()),
            2,
        ),
    ] {
        assert_eq!(
            call(&value, None, WeekContext::default(), context),
            Ok(Value::Int16(expected))
        );
    }
    assert_eq!(
        call(
            &Value::Text("2024-02-29".into()),
            None,
            WeekContext::default(),
            DateParseContext::default()
        ),
        Err(EvalError::DateFormatRequired)
    );
    assert_eq!(
        call(
            &Value::Double(Double::from_float(f64::NAN)),
            None,
            WeekContext::default(),
            context
        ),
        Err(EvalError::NonFinite)
    );
    let legacy = legacy("2024-02-29T23:59:59.5");
    let extended =
        Value::DateTimeExtended(ExtendedDateTime::new(2024, 2, 29, 23, 59, 59, 5_000_000).unwrap());
    assert_eq!(
        call(&legacy, None, WeekContext::default(), context),
        Ok(Value::Int16(6))
    );
    assert_eq!(
        call(&extended, None, WeekContext::default(), context),
        Ok(Value::Int16(5))
    );
}
#[test]
fn callback_failure_is_never_a_weekday_or_null_result() {
    for failure in [
        EvalError::Cancelled,
        EvalError::Allocation,
        EvalError::TextLimit,
        EvalError::Overflow,
    ] {
        assert_eq!(
            weekday(
                &Value::Null,
                None,
                WeekContext::default(),
                DateParseContext::default(),
                &mut Buffer {
                    failure: Some(failure),
                    ..Default::default()
                }
            ),
            Err(failure)
        );
    }
}
proptest! {
    #![proptest_config(crate::test_support::config())]
    #[test]
    fn serial_whole_days_follow_the_independent_seven_day_cycle(day in -657434_i32..=2958465, first in 1_i16..=7) {
        let date = Value::DateTime(LegacyDateTime::from_ole_days(f64::from(day)).unwrap());
        let expected = ((day + 6 - i32::from(first - 1)).rem_euclid(7) + 1) as i16;
        prop_assert_eq!(call(&date, Some(&Value::Int16(first)), WeekContext::default(), DateParseContext::default()), Ok(Value::Int16(expected)));
    }
}

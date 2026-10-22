use super::*;
use crate::functions::{
    dates::WeekContext,
    text::{TextContext, TextResult, format::prepare},
};
use proptest::prelude::*;
use tablecraft_core::values::{ExtendedDateTime, LegacyDateTime};

#[derive(Default)]
struct Buffer(String);

impl TextOutput for Buffer {
    fn step(&mut self) -> Result<(), EvalError> {
        Ok(())
    }

    fn append(&mut self, text: &str) -> Result<(), EvalError> {
        self.0.push_str(text);
        Ok(())
    }
}

impl NumericBuffer for Buffer {
    fn text(&self) -> &str {
        &self.0
    }
}

fn date() -> Value {
    Value::DateTimeExtended(ExtendedDateTime::new(1993, 1, 27, 17, 4, 23, 0).unwrap())
}

fn render(value: Value, mask: &str, profile: Option<DateTextFormat>) -> Result<String, EvalError> {
    let args = [value, Value::Text(mask.into())];
    let TextResult::Format(recipe) = prepare(
        &args,
        TextContext {
            date_format: profile,
            ..Default::default()
        },
    )?
    else {
        panic!("expected Format recipe");
    };
    let display = recipe.resolve(&mut Buffer::default())?;
    let mut output = Buffer::default();
    display.write(&mut output)?;
    Ok(output.0)
}

#[test]
fn custom_date_tokens_and_hour_context_preserve_literal_boundaries() {
    for (mask, expected) in [
        ("yyyy-mm-dd", "1993-01-27"),
        ("h:m:s", "17:4:23"),
        ("hh:mm:ss am/pm", "05:04:23 pm"),
        ("hh:nn:ss AM/PM", "05:04:23 PM"),
        ("h:mm A/P", "5:04 P"),
        ("h:mm a/p", "5:04 p"),
        ("dddd, mmm d yyyy", "Wednesday, Jan 27 1993"),
        ("d dd m mm yy yyyy q y w ww", "27 27 1 01 93 1993 1 27 4 5"),
        (r#"yyyy-mm-dd "at" hh:nn:ss"#, "1993-01-27 at 17:04:23"),
        (r#"yyyy";mm:AM/PM""#, "1993;mm:AM/PM"),
        ("hh:nn mmmm", "17:04 January"),
        (r#"hh "AM/PM""#, "17 AM/PM"),
    ] {
        assert_eq!(
            render(date(), mask, Some(DateTextFormat::EnUs)),
            Ok(expected.into()),
            "{mask}"
        );
    }
    assert_eq!(render(date(), "yyyy-mm-dd", None), Ok("1993-01-27".into()));
    assert_eq!(render(date(), r"hh\:nn\:ss", None), Ok("17:04:23".into()));
    assert_eq!(
        render(date(), "m/d/yyyy", Some(DateTextFormat::DeDe)),
        Ok("1.27.1993".into())
    );
}

#[test]
fn named_styles_and_regional_tokens_reuse_explicit_date_profiles() {
    for (mask, expected) in [
        ("General Date", "1/27/1993 5:04:23 PM"),
        ("Long Date", "Wednesday, January 27, 1993"),
        ("Medium Date", "27-Jan-93"),
        ("Short Date", "1/27/1993"),
        ("Long Time", "5:04:23 PM"),
        ("Medium Time", "05:04 PM"),
        ("Short Time", "17:04"),
        ("c", "1/27/1993 5:04:23 PM"),
        ("ddddd", "1/27/1993"),
        ("dddddd", "Wednesday, January 27, 1993"),
        ("ttttt", "5:04:23 PM"),
        ("hh:nn AMPM", "05:04 PM"),
    ] {
        assert_eq!(
            render(date(), mask, Some(DateTextFormat::EnUs)),
            Ok(expected.into()),
            "{mask}"
        );
    }
    assert_eq!(
        render(date(), "dddd, d. mmmm yyyy", Some(DateTextFormat::DeDe)),
        Ok("Mittwoch, 27. Januar 1993".into())
    );
    assert_eq!(render(date(), "Short Time", None), Ok("17:04".into()));
    for mask in ["hh:nn AMPM", "Medium Time"] {
        assert_eq!(
            render(date(), mask, Some(DateTextFormat::DeDe)),
            Err(EvalError::UnsupportedExpression)
        );
    }
}

#[test]
fn date_input_rounding_ranges_and_intrinsic_policy_boundaries_are_checked() {
    assert_eq!(
        render(Value::Int32(0), "yyyy-mm-dd", None),
        Ok("1899-12-30".into())
    );
    assert_eq!(
        render(
            Value::Text("1993-01-27 17:04:23".into()),
            "yyyy-mm-dd",
            Some(DateTextFormat::Iso)
        ),
        Ok("1993-01-27".into())
    );
    assert_eq!(
        render(Value::Text("1993-01-27".into()), "yyyy-mm-dd", None),
        Err(EvalError::DateFormatRequired)
    );
    assert_eq!(
        render(
            Value::DateTime(LegacyDateTime::from_ole_days(-1.25).unwrap()),
            "yyyy-mm-dd hh:nn",
            Some(DateTextFormat::Iso)
        ),
        Ok("1899-12-29 06:00".into())
    );
    let boundary =
        Value::DateTimeExtended(ExtendedDateTime::new(2024, 2, 29, 23, 59, 59, 5_000_000).unwrap());
    assert_eq!(
        render(boundary, "yyyy-mm-dd hh:nn:ss", Some(DateTextFormat::Iso)),
        Ok("2024-03-01 00:00:00".into())
    );
    for mask in [
        "ddd",
        "mmmm",
        "m/d/yyyy",
        "hh:nn:ss",
        "AMPM",
        "General Date",
    ] {
        assert_eq!(
            render(date(), mask, None),
            Err(EvalError::DateFormatRequired),
            "{mask}"
        );
    }
    for mask in ["yyy", "dddddDD", "hhhhh", "AM", "yyyy;yyyy", "yyyy0"] {
        assert_eq!(
            render(date(), mask, Some(DateTextFormat::Iso)),
            Err(EvalError::UnsupportedExpression),
            "{mask}"
        );
    }
}

proptest! {
    #![proptest_config(crate::test_support::config())]

    #[test]
    fn intrinsic_numeric_date_masks_match_component_spelling(
        year in 100u16..10_000,
        month in 1u8..13,
        day in 1u8..29,
        hour in 0u8..24,
        minute in 0u8..60,
        second in 0u8..60,
    ) {
        let value = Value::DateTimeExtended(ExtendedDateTime::new(year, month, day, hour, minute, second, 0).unwrap());
        let expected = format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}");
        prop_assert_eq!(render(value, r"yyyy-mm-dd hh\:nn\:ss", None).unwrap(), expected);
    }
}

#[test]
fn format_week_fields_share_the_documented_legacy_monday_rule() {
    let weeks = WeekSettings::parse(
        Some(&Value::Int32(2)),
        Some(&Value::Int32(2)),
        WeekContext::default(),
    )
    .unwrap();
    let mut scratch = Buffer::default();
    let pattern = Pattern::parse("w ww", &mut || Ok(())).unwrap().unwrap();
    let value = Value::DateTimeExtended(ExtendedDateTime::new(2018, 12, 31, 0, 0, 0, 0).unwrap());
    let display = pattern
        .resolve(&value, DateParseContext::default(), weeks, &mut scratch)
        .unwrap();
    let mut output = Buffer::default();
    display.write(&mut output).unwrap();
    assert_eq!(output.0, "1 53");
}

#[test]
fn extended_year_boundaries_and_subsecond_day_carry_remain_explicit() {
    let earliest = Value::DateTimeExtended(ExtendedDateTime::new(1, 1, 1, 0, 0, 0, 0).unwrap());
    assert_eq!(
        render(earliest, "yyyy-mm-dd", None),
        Ok("0001-01-01".into())
    );
    let latest = Value::DateTimeExtended(
        ExtendedDateTime::new(9999, 12, 31, 23, 59, 59, 5_000_000).unwrap(),
    );
    assert_eq!(render(latest, "yyyy-mm-dd", None), Err(EvalError::Overflow));
}

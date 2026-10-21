use super::*;
use crate::functions::text::{TextFunction, TextResult, apply_with_context};
use tablecraft_core::values::{ExtendedDateTime, LegacyDateTime};
#[derive(Default)]
struct Buffer {
    text: String,
}
impl TextOutput for Buffer {
    fn step(&mut self) -> Result<(), EvalError> {
        Ok(())
    }
    fn append(&mut self, value: &str) -> Result<(), EvalError> {
        self.text.push_str(value);
        Ok(())
    }
}
impl NumericBuffer for Buffer {
    fn text(&self) -> &str {
        &self.text
    }
}
fn call(value: Value, mode: i32, profile: DateTextFormat) -> Result<String, EvalError> {
    let arguments = [value, Value::Int32(mode)];
    let TextResult::DateTimeFormat(format) = apply_with_context(
        TextFunction::FormatDateTime,
        &arguments,
        TextContext {
            date_format: Some(profile),
            ..Default::default()
        },
        || Ok(()),
    )?
    else {
        panic!()
    };
    let display = format.resolve(&mut Buffer::default())?;
    let mut output = Buffer::default();
    display.write(&mut output)?;
    Ok(output.text)
}
fn value() -> Value {
    Value::DateTimeExtended(ExtendedDateTime::new(2026, 10, 20, 16, 28, 14, 0).unwrap())
}
#[test]
fn named_formats_render_all_profiles_with_intrinsic_24_hour_short_time() {
    for (profile, expected) in [
        (
            DateTextFormat::Iso,
            [
                "2026-10-20 16:28:14",
                "2026-10-20",
                "2026-10-20",
                "16:28:14",
                "16:28",
            ],
        ),
        (
            DateTextFormat::EnUs,
            [
                "10/20/2026 4:28:14 PM",
                "Tuesday, October 20, 2026",
                "10/20/2026",
                "4:28:14 PM",
                "16:28",
            ],
        ),
        (
            DateTextFormat::DeDe,
            [
                "20.10.2026 16:28:14",
                "Dienstag, 20. Oktober 2026",
                "20.10.2026",
                "16:28:14",
                "16:28",
            ],
        ),
    ] {
        for (mode, expected) in expected.into_iter().enumerate() {
            assert_eq!(call(value(), mode as i32, profile).unwrap(), expected);
        }
    }
    let epoch = Value::DateTime(LegacyDateTime::from_ole_days(0.0).unwrap());
    assert_eq!(
        call(epoch.clone(), 0, DateTextFormat::EnUs).unwrap(),
        "12:00:00 AM"
    );
    assert_eq!(call(epoch, 2, DateTextFormat::EnUs).unwrap(), "12/30/1899");
}
#[test]
fn conversions_rounding_and_selector_failures_remain_checked() {
    assert_eq!(
        call(Value::Text("04.28.14".into()), 4, DateTextFormat::EnUs).unwrap(),
        "04:28"
    );
    assert_eq!(
        call(
            Value::Double(tablecraft_core::values::Double::from_float(-1.25)),
            0,
            DateTextFormat::Iso
        )
        .unwrap(),
        "1899-12-29 06:00:00"
    );
    assert_eq!(
        call(Value::Null, 0, DateTextFormat::Iso),
        Err(EvalError::InvalidNullConversion)
    );
    for mode in [-1, 5] {
        assert_eq!(
            call(value(), mode, DateTextFormat::Iso),
            Err(EvalError::Domain)
        );
    }
    let carry =
        Value::DateTimeExtended(ExtendedDateTime::new(2024, 2, 29, 23, 59, 59, 5_000_000).unwrap());
    assert_eq!(call(carry, 0, DateTextFormat::Iso).unwrap(), "2024-03-01");
}

#[test]
fn scratch_is_private_and_intrinsic_short_time_needs_no_output_profile() {
    let arguments = [Value::Text("-1.25".into())];
    let TextResult::DateTimeFormat(recipe) = apply_with_context(
        TextFunction::FormatDateTime,
        &arguments,
        TextContext {
            date_format: Some(DateTextFormat::Iso),
            number_format: Some(crate::functions::conversions::text::NumberFormat::Invariant),
            ..Default::default()
        },
        || Ok(()),
    )
    .unwrap() else {
        panic!()
    };
    let mut scratch = Buffer::default();
    let display = recipe.resolve(&mut scratch).unwrap();
    assert!(!scratch.text.is_empty());
    let mut output = Buffer::default();
    display.write(&mut output).unwrap();
    assert_eq!(output.text, "1899-12-29 06:00:00");
    let arguments = [value(), Value::Int16(4)];
    let TextResult::DateTimeFormat(recipe) = apply_with_context(
        TextFunction::FormatDateTime,
        &arguments,
        TextContext::default(),
        || Ok(()),
    )
    .unwrap() else {
        panic!()
    };
    let display = recipe.resolve(&mut Buffer::default()).unwrap();
    let mut output = Buffer::default();
    display.write(&mut output).unwrap();
    assert_eq!(output.text, "16:28");
    let arguments = [Value::Text("2026-10-20".into()), Value::Int16(4)];
    let TextResult::DateTimeFormat(recipe) = apply_with_context(
        TextFunction::FormatDateTime,
        &arguments,
        TextContext::default(),
        || Ok(()),
    )
    .unwrap() else {
        panic!()
    };
    assert_eq!(
        recipe.resolve(&mut Buffer::default()),
        Err(EvalError::DateFormatRequired)
    );
}
#[test]
fn named_selectors_coerce_half_even_and_long_dates_fit_fixed_storage() {
    let arguments = [
        value(),
        Value::Double(tablecraft_core::values::Double::from_float(3.5)),
    ];
    let recipe = prepare(&arguments, TextContext::default()).unwrap();
    assert_eq!(recipe.mode, NamedDateFormat::ShortTime);
    for month in 1..=12 {
        for day in 1..=28 {
            let date = Value::DateTimeExtended(
                ExtendedDateTime::new(2026, month, day, 0, 0, 0, 0).unwrap(),
            );
            for profile in [DateTextFormat::EnUs, DateTextFormat::DeDe] {
                assert!(!call(date.clone(), 1, profile).unwrap().is_empty());
            }
        }
    }
}

#[test]
fn scratch_and_output_provider_failures_are_never_successful_results() {
    struct Failing(EvalError);
    impl TextOutput for Failing {
        fn step(&mut self) -> Result<(), EvalError> {
            Err(self.0)
        }
        fn append(&mut self, _: &str) -> Result<(), EvalError> {
            Err(self.0)
        }
    }
    impl NumericBuffer for Failing {
        fn text(&self) -> &str {
            ""
        }
    }
    let arguments = [value()];
    let recipe = prepare(
        &arguments,
        TextContext {
            date_format: Some(DateTextFormat::EnUs),
            ..Default::default()
        },
    )
    .unwrap();
    let display = recipe.resolve(&mut Buffer::default()).unwrap();
    for error in [
        EvalError::Cancelled,
        EvalError::TextLimit,
        EvalError::Allocation,
        EvalError::TypeMismatch,
        EvalError::Overflow,
    ] {
        assert_eq!(recipe.resolve(&mut Failing(error)), Err(error));
        assert_eq!(display.write(&mut Failing(error)), Err(error));
    }
}

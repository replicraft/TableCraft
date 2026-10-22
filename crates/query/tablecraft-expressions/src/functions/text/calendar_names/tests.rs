use super::*;
use crate::functions::text::{TextResult, apply_with_context};
use tablecraft_core::values::{Double, ExtendedDateTime, LegacyDateTime};
#[derive(Default)]
struct Output {
    text: String,
    failure: Option<EvalError>,
}
impl TextOutput for Output {
    fn step(&mut self) -> Result<(), EvalError> {
        self.failure.map_or(Ok(()), Err)
    }
    fn append(&mut self, text: &str) -> Result<(), EvalError> {
        self.step()?;
        self.text.push_str(text);
        Ok(())
    }
}
fn call(
    function: TextFunction,
    args: &[Value],
    profile: DateTextFormat,
    first: Option<u8>,
) -> Result<String, EvalError> {
    let TextResult::CalendarName(name) = apply_with_context(
        function,
        args,
        TextContext {
            date_format: Some(profile),
            first_day_of_week: first.map(|day| FirstDayOfWeek::new(day).unwrap()),
            ..Default::default()
        },
        || Ok(()),
    )?
    else {
        panic!()
    };
    let mut output = Output::default();
    name.write(&mut output)?;
    Ok(output.text)
}
#[test]
fn months_cover_full_and_abbreviated_names_in_explicit_profiles() {
    for (profile, names, short) in [
        (
            DateTextFormat::EnUs,
            "January February March April May June July August September October November December",
            "Jan Feb Mar Apr May Jun Jul Aug Sep Oct Nov Dec",
        ),
        (
            DateTextFormat::Iso,
            "January February March April May June July August September October November December",
            "Jan Feb Mar Apr May Jun Jul Aug Sep Oct Nov Dec",
        ),
        (
            DateTextFormat::DeDe,
            "Januar Februar März April Mai Juni Juli August September Oktober November Dezember",
            "Jan Feb Mär Apr Mai Jun Jul Aug Sep Okt Nov Dez",
        ),
    ] {
        for (month, (name, abbreviation)) in names
            .split_whitespace()
            .zip(short.split_whitespace())
            .enumerate()
        {
            assert_eq!(
                call(
                    TextFunction::MonthName,
                    &[Value::Int32(month as i32 + 1)],
                    profile,
                    None
                )
                .unwrap(),
                name
            );
            assert_eq!(
                call(
                    TextFunction::MonthName,
                    &[Value::Int32(month as i32 + 1), Value::Boolean(true)],
                    profile,
                    None
                )
                .unwrap(),
                abbreviation
            );
        }
    }
}
#[test]
fn weekday_numbering_wraps_all_first_days_and_system_context_is_not_inferred() {
    for (profile, full, abbreviated) in [
        (
            DateTextFormat::EnUs,
            "Sunday Monday Tuesday Wednesday Thursday Friday Saturday",
            "Sun Mon Tue Wed Thu Fri Sat",
        ),
        (
            DateTextFormat::DeDe,
            "Sonntag Montag Dienstag Mittwoch Donnerstag Freitag Samstag",
            "So Mo Di Mi Do Fr Sa",
        ),
    ] {
        let full = full.split_whitespace().collect::<Vec<_>>();
        let abbreviated = abbreviated.split_whitespace().collect::<Vec<_>>();
        for first in 1..=7 {
            for day in 1..=7 {
                let expected = ((first - 1 + day - 1) % 7) as usize;
                for (short, names) in [(false, &full), (true, &abbreviated)] {
                    assert_eq!(
                        call(
                            TextFunction::WeekdayName,
                            &[
                                Value::Int32(day),
                                Value::Boolean(short),
                                Value::Int32(first)
                            ],
                            profile,
                            None
                        )
                        .unwrap(),
                        names[expected]
                    );
                    assert_eq!(
                        call(
                            TextFunction::WeekdayName,
                            &[Value::Int32(day), Value::Boolean(short)],
                            profile,
                            Some(first as u8)
                        )
                        .unwrap(),
                        names[expected]
                    );
                }
            }
        }
    }
    assert_eq!(
        call(
            TextFunction::WeekdayName,
            &[Value::Int16(1)],
            DateTextFormat::EnUs,
            None
        ),
        Err(EvalError::CalendarPolicyRequired)
    );
    assert_eq!(
        call(
            TextFunction::WeekdayName,
            &[Value::Int16(1), Value::Boolean(false), Value::Int16(0)],
            DateTextFormat::EnUs,
            Some(2)
        )
        .unwrap(),
        "Monday"
    );
    assert_eq!(
        call(
            TextFunction::WeekdayName,
            &[Value::Int16(1), Value::Boolean(false), Value::Int16(1)],
            DateTextFormat::EnUs,
            Some(2)
        )
        .unwrap(),
        "Sunday"
    );
}
#[test]
fn coercions_null_and_domain_failures_remain_explicit() {
    assert_eq!(
        call(
            TextFunction::MonthName,
            &[Value::Double(Double::from_float(1.5)), Value::Int16(1)],
            DateTextFormat::EnUs,
            None
        )
        .unwrap(),
        "Feb"
    );
    assert_eq!(
        call(
            TextFunction::MonthName,
            &[Value::DateTime(LegacyDateTime::from_ole_days(2.0).unwrap())],
            DateTextFormat::EnUs,
            None
        ),
        Ok("February".into())
    );
    for (function, args, expected) in [
        (
            TextFunction::MonthName,
            vec![Value::Null],
            EvalError::InvalidNullConversion,
        ),
        (
            TextFunction::MonthName,
            vec![Value::Int16(1), Value::Null],
            EvalError::InvalidNullConversion,
        ),
        (
            TextFunction::MonthName,
            vec![Value::Int16(0)],
            EvalError::Domain,
        ),
        (
            TextFunction::MonthName,
            vec![Value::Int16(13)],
            EvalError::Domain,
        ),
        (
            TextFunction::WeekdayName,
            vec![Value::Int16(8)],
            EvalError::Domain,
        ),
        (
            TextFunction::WeekdayName,
            vec![Value::Int16(1), Value::Boolean(false), Value::Int16(8)],
            EvalError::Domain,
        ),
        (
            TextFunction::MonthName,
            vec![Value::Text("1".into())],
            EvalError::TypeMismatch,
        ),
        (
            TextFunction::MonthName,
            vec![Value::DateTimeExtended(
                ExtendedDateTime::new(2025, 1, 1, 0, 0, 0, 0).unwrap(),
            )],
            EvalError::TypeMismatch,
        ),
    ] {
        assert_eq!(
            call(function, &args, DateTextFormat::EnUs, None),
            Err(expected)
        );
    }
    let arguments = [Value::Int16(1)];
    assert_eq!(
        apply_with_context(
            TextFunction::MonthName,
            &arguments,
            TextContext::default(),
            || Ok(())
        ),
        Err(EvalError::DateFormatRequired)
    );
}
#[test]
fn callback_failures_are_not_successful_names() {
    let arguments = [Value::Int16(3)];
    for profile in [DateTextFormat::EnUs, DateTextFormat::DeDe] {
        let name = prepare(
            TextFunction::MonthName,
            &arguments,
            TextContext {
                date_format: Some(profile),
                ..Default::default()
            },
        )
        .unwrap();
        for error in [
            EvalError::Cancelled,
            EvalError::Allocation,
            EvalError::TextLimit,
            EvalError::TypeMismatch,
        ] {
            assert_eq!(
                name.write(&mut Output {
                    failure: Some(error),
                    ..Default::default()
                }),
                Err(error)
            );
        }
    }
}

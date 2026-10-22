use super::*;
use jiff::ToSpan;
use proptest::prelude::*;
use tablecraft_core::values::{Decimal, Double};
fn run(function: DateConstructor, values: [i32; 3]) -> Result<Value, EvalError> {
    function.apply(&values.map(Value::Int32))
}
fn calendar(value: Value) -> String {
    let Value::DateTime(date) = value else {
        panic!("Date required")
    };
    date.calendar_rounded_to_nanos().unwrap().to_string()
}
#[test]
fn dates_normalize_relative_units_and_use_an_explicit_year_window() {
    for (values, expected) in [
        ([1969, 2, 12], "1969-02-12T00:00:00"),
        ([1990 - 10, 8 - 2, 1 - 1], "1980-05-31T00:00:00"),
        ([2024, 3, 0], "2024-02-29T00:00:00"),
        ([2024, 13, 1], "2025-01-01T00:00:00"),
        ([2024, 0, 1], "2023-12-01T00:00:00"),
        ([0, 1, 1], "2000-01-01T00:00:00"),
        ([29, 1, 1], "2029-01-01T00:00:00"),
        ([30, 1, 1], "1930-01-01T00:00:00"),
        ([99, 1, 1], "1999-01-01T00:00:00"),
        ([10050, 1, -32768], "9960-04-14T00:00:00"),
    ] {
        assert_eq!(
            calendar(run(DateConstructor::DateSerial, values).unwrap()),
            expected
        );
    }
    let arguments = [Value::Int32(49), Value::Int32(1), Value::Int32(1)];
    assert_eq!(
        calendar(
            DateConstructor::DateSerial
                .apply_with_window(&arguments, TwoDigitYearWindow::new(2049).unwrap())
                .unwrap()
        ),
        "2049-01-01T00:00:00"
    );
    for last in [0, 198, 10000] {
        assert_eq!(TwoDigitYearWindow::new(last), Err(EvalError::Domain));
    }
}
#[test]
fn times_retain_signed_serial_carries_and_int16_half_even_arguments() {
    for (values, expected) in [
        ([6, -15, 0], 5.75 / 24.0),
        ([0, 75, 0], 1.25 / 24.0),
        ([25, 0, 0], 25.0 / 24.0),
        ([-1, 0, 0], -1.0 / 24.0),
        ([0, 0, -1], -1.0 / 86400.0),
    ] {
        let Value::DateTime(value) = run(DateConstructor::TimeSerial, values).unwrap() else {
            panic!("Date required")
        };
        assert_eq!(value.ole_days(), expected);
    }
    assert_eq!(
        DateConstructor::DateSerial.apply(&[
            Value::Int32(2024),
            Value::Decimal(Decimal::new(25, 1).unwrap()),
            Value::Double(Double::from_float(28.5))
        ]),
        run(DateConstructor::DateSerial, [2024, 2, 28])
    );
    assert_eq!(
        DateConstructor::TimeSerial.apply(&[
            Value::Boolean(true),
            Value::Int32(0),
            Value::Int32(0)
        ]),
        run(DateConstructor::TimeSerial, [-1, 0, 0])
    );
}
#[test]
fn null_ranges_wrong_types_and_invalid_calendars_fail_without_defaults() {
    for function in [DateConstructor::DateSerial, DateConstructor::TimeSerial] {
        assert_eq!(function.apply(&[]), Err(EvalError::UnsupportedExpression));
        for value in [Value::Int32(-32769), Value::Int32(32768)] {
            assert_eq!(
                function.apply(&[value, Value::Int32(1), Value::Int32(1)]),
                Err(EvalError::Overflow)
            );
        }
        assert_eq!(
            function.apply(&[Value::Null, Value::Int32(1), Value::Int32(1)]),
            Err(EvalError::InvalidNullConversion)
        );
        assert_eq!(
            function.apply(&[Value::Text("1".into()), Value::Int32(1), Value::Int32(1)]),
            Err(EvalError::TypeMismatch)
        );
    }
    for values in [
        [100, 1, 0],
        [9999, 12, 32],
        [-1, 1, 1],
        [32767, 32767, 32767],
    ] {
        assert_eq!(
            run(DateConstructor::DateSerial, values),
            Err(EvalError::Overflow)
        );
    }
}
proptest! {
    #![proptest_config(proptest::test_runner::Config { failure_persistence: None, ..Default::default() })]
    #[test]
    fn relative_dates_match_separate_jiff_month_then_day_arithmetic(year in 200_i16..9000_i16, month in -24_i16..24_i16, day in -400_i16..400_i16) {
        let reference = Date::new(year, 1, 1).unwrap()
            .checked_add((i64::from(month) - 1).months()).unwrap()
            .checked_add((i64::from(day) - 1).days()).unwrap();
        let expected = Value::DateTime(
            LegacyDateTime::from_calendar(reference.to_datetime(Time::MIN)).unwrap(),
        );
        let components = [i32::from(year), i32::from(month), i32::from(day)];
        prop_assert_eq!(run(DateConstructor::DateSerial, components), Ok(expected));
    }
    #[test]
    fn signed_times_recover_independent_integer_seconds(hour in any::<i16>(), minute in any::<i16>(), second in any::<i16>()) {
        let components = [i32::from(hour), i32::from(minute), i32::from(second)];
        let Value::DateTime(value) = run(DateConstructor::TimeSerial, components).unwrap() else {
            unreachable!()
        };
        let expected = i64::from(hour) * 3600 + i64::from(minute) * 60 + i64::from(second);
        prop_assert_eq!((value.ole_days() * 86400.0).round() as i64, expected);
    }
}

#[test]
fn raw_date_components_use_checked_integer_conversion_without_calendar_projection() {
    let year = Value::DateTime(LegacyDateTime::from_ole_days(2024.5).unwrap());
    let month = Value::DateTime(LegacyDateTime::from_ole_days(2.5).unwrap());
    let day = Value::DateTime(LegacyDateTime::from_ole_days(28.5).unwrap());
    assert_eq!(
        calendar(
            DateConstructor::DateSerial
                .apply(&[year, month, day])
                .unwrap()
        ),
        "2024-02-28T00:00:00"
    );
    let negative = Value::DateTime(LegacyDateTime::from_ole_days(-1.5).unwrap());
    assert_eq!(
        DateConstructor::TimeSerial.apply(&[negative, Value::Int16(0), Value::Int16(0)]),
        Ok(Value::DateTime(
            LegacyDateTime::from_ole_days(-2.0 / 24.0).unwrap()
        ))
    );
}

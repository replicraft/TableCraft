use super::*;
use proptest::prelude::*;
use tablecraft_core::values::{ExtendedDateTime, LegacyDateTime};
fn legacy(text: &str) -> Value {
    Value::DateTime(LegacyDateTime::from_calendar(text.parse().unwrap()).unwrap())
}
fn diff(interval: &str, first: &Value, second: &Value) -> Result<Value, EvalError> {
    date_diff(&Value::Text(interval.into()), first, second, None, None)
}
#[test]
fn calendar_boundaries_are_not_elapsed_whole_units() {
    for (interval, start, end, expected) in [
        ("yyyy", "2025-12-31T23:59:59", "2026-01-01T00:00:00", 1),
        ("q", "2026-03-31T23:59:59", "2026-04-01T00:00:00", 1),
        ("m", "2026-01-31T23:59:59", "2026-02-01T00:00:00", 1),
        ("y", "2024-02-28T12:00:00", "2024-03-01T00:00:00", 2),
        ("d", "2026-01-01T23:59:59", "2026-01-02T00:00:00", 1),
        ("w", "2026-01-05T12:00:00", "2026-01-11T00:00:00", 0),
        ("w", "2026-01-05T12:00:00", "2026-01-12T00:00:00", 1),
        ("ww", "2026-01-05T12:00:00", "2026-01-11T00:00:00", 1),
        ("h", "2026-01-01T12:59:59", "2026-01-01T13:00:00", 1),
        ("n", "2026-01-01T12:00:59", "2026-01-01T12:01:00", 1),
        ("s", "1899-12-29T23:59:59", "1899-12-30T00:00:00", 1),
    ] {
        let first = legacy(start);
        let second = legacy(end);
        assert_eq!(diff(interval, &first, &second), Ok(Value::Int32(expected)));
        assert_eq!(diff(interval, &second, &first), Ok(Value::Int32(-expected)));
    }
}
#[test]
fn week_options_do_not_infer_system_settings_or_convert_null_to_omission() {
    let first = legacy("2026-01-04T12:00:00");
    let second = legacy("2026-01-05T12:00:00");
    let interval = Value::Text("ww".into());
    assert_eq!(
        date_diff(&interval, &first, &second, None, None),
        Ok(Value::Int32(0))
    );
    for rule in 1..=3 {
        assert_eq!(
            date_diff(
                &interval,
                &first,
                &second,
                Some(&Value::Int32(2)),
                Some(&Value::Int32(rule))
            ),
            Ok(Value::Int32(1))
        );
    }
    for (setting, error) in [
        (Value::Int32(0), EvalError::CalendarPolicyRequired),
        (Value::Int32(8), EvalError::Domain),
        (Value::Null, EvalError::TypeMismatch),
    ] {
        assert_eq!(
            date_diff(&interval, &first, &second, Some(&setting), None),
            Err(error)
        );
    }
    assert_eq!(
        date_diff(&interval, &first, &second, None, Some(&Value::Int32(4))),
        Err(EvalError::Domain)
    );
    assert_eq!(
        date_diff(
            &Value::Text("w".into()),
            &first,
            &second,
            Some(&Value::Int32(2)),
            None
        ),
        Err(EvalError::UnsupportedExpression)
    );
}
#[test]
fn extended_boundaries_nulls_type_and_long_overflow_are_explicit() {
    let first =
        Value::DateTimeExtended(ExtendedDateTime::new(1, 1, 1, 0, 0, 0, 9_999_999).unwrap());
    let second = Value::DateTimeExtended(ExtendedDateTime::new(1, 1, 1, 0, 0, 1, 0).unwrap());
    assert_eq!(diff("s", &first, &second), Ok(Value::Int32(1)));
    let last = Value::DateTimeExtended(
        ExtendedDateTime::new(9999, 12, 31, 23, 59, 59, 9_999_999).unwrap(),
    );
    assert_eq!(diff("yyyy", &first, &last), Ok(Value::Int32(9998)));
    assert_eq!(diff("s", &first, &last), Err(EvalError::Overflow));
    assert_eq!(diff("d", &first, &Value::Null), Ok(Value::Null));
    assert_eq!(diff("d", &Value::Null, &last), Ok(Value::Null));
    assert_eq!(
        diff("d", &Value::Int64(0), &last),
        Err(EvalError::TypeMismatch)
    );
    assert_eq!(
        diff("d", &Value::Text("not a date".into()), &Value::Null),
        Err(EvalError::TypeMismatch)
    );
    assert_eq!(
        diff("invalid", &first, &last),
        Err(EvalError::InvalidDateInterval)
    );
}
proptest! {
    #![proptest_config(crate::test_support::config())]
    #[test]
    fn signed_day_counts_match_independent_integer_offsets(first in -100_000_i32..=100_000,second in -100_000_i32..=100_000) {
        let a=Value::DateTime(LegacyDateTime::from_ole_days(f64::from(first)).unwrap());
        let b=Value::DateTime(LegacyDateTime::from_ole_days(f64::from(second)).unwrap());
        prop_assert_eq!(diff("d",&a,&b),Ok(Value::Int32(second-first)));
        prop_assert_eq!(diff("w",&a,&b),Ok(Value::Int32((second-first)/7)));
    }
}

#[test]
fn host_first_day_changes_only_explicit_system_week_boundary_requests() {
    use super::super::{FirstDayOfWeek, FirstWeekOfYear};
    let first = Value::DateTime(
        LegacyDateTime::from_calendar("2026-10-18T12:00:00".parse().unwrap()).unwrap(),
    );
    let second = Value::DateTime(
        LegacyDateTime::from_calendar("2026-10-19T12:00:00".parse().unwrap()).unwrap(),
    );
    let zero = Value::Int16(0);
    let one = Value::Int16(1);
    let context = WeekContext {
        first_day: Some(FirstDayOfWeek::new(2).unwrap()),
        first_week: Some(FirstWeekOfYear::new(2).unwrap()),
    };
    let interval = Value::Text("ww".into());
    assert_eq!(
        date_diff_with_context(
            &interval,
            &first,
            &second,
            Some(&zero),
            Some(&zero),
            context
        ),
        Ok(Value::Int32(1))
    );
    assert_eq!(
        date_diff_with_context(
            &interval,
            &second,
            &first,
            Some(&zero),
            Some(&zero),
            context
        ),
        Ok(Value::Int32(-1))
    );
    assert_eq!(
        date_diff_with_context(&interval, &first, &second, None, None, context),
        Ok(Value::Int32(0))
    );
    assert_eq!(
        date_diff_with_context(&interval, &first, &second, Some(&one), Some(&one), context),
        Ok(Value::Int32(0))
    );
    assert_eq!(
        date_diff(&interval, &first, &second, Some(&zero), Some(&zero)),
        Err(EvalError::CalendarPolicyRequired)
    );
    assert_eq!(
        date_diff_with_context(
            &Value::Text("w".into()),
            &first,
            &second,
            Some(&zero),
            None,
            context
        ),
        Err(EvalError::UnsupportedExpression)
    );
}

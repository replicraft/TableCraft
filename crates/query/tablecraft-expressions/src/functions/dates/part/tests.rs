use super::*;
use proptest::prelude::*;
use tablecraft_core::values::{ExtendedDateTime, LegacyDateTime};
fn legacy(text: &str) -> Value {
    Value::DateTime(LegacyDateTime::from_calendar(text.parse().unwrap()).unwrap())
}
fn part(
    interval: &str,
    date: &Value,
    first: Option<i32>,
    week: Option<i32>,
) -> Result<Value, EvalError> {
    date_part(
        &Value::Text(interval.into()),
        date,
        first.map(Value::Int32).as_ref(),
        week.map(Value::Int32).as_ref(),
    )
}
#[test]
fn all_intervals_use_distinct_date_parts_and_exact_integer_carriers() {
    let date = legacy("2024-02-29T12:34:56");
    for (interval, expected) in [
        ("yyyy", 2024),
        ("q", 1),
        ("m", 2),
        ("y", 60),
        ("d", 29),
        ("w", 5),
        ("ww", 9),
        ("h", 12),
        ("n", 34),
        ("s", 56),
    ] {
        assert_eq!(
            part(interval, &date, None, None),
            Ok(Value::Int16(expected))
        );
        assert_eq!(part(interval, &Value::Null, None, None), Ok(Value::Null));
    }
    assert_eq!(part("w", &date, Some(2), None), Ok(Value::Int16(4)));
    let precise =
        Value::DateTimeExtended(ExtendedDateTime::new(1, 1, 1, 23, 59, 59, 9_999_999).unwrap());
    assert_eq!(part("yyyy", &precise, None, None), Ok(Value::Int16(1)));
    assert_eq!(part("s", &precise, None, None), Ok(Value::Int16(59)));
}
#[test]
fn week_rules_cross_years_and_preserve_documented_final_monday_behavior() {
    for (text, expected) in [
        ("2003-12-28T12:00:00", 52),
        ("2003-12-29T12:00:00", 53),
        ("2003-12-30T12:00:00", 1),
        ("2003-12-31T12:00:00", 1),
        ("2021-01-01T12:00:00", 53),
        ("2022-01-01T12:00:00", 52),
    ] {
        assert_eq!(
            part("ww", &legacy(text), Some(2), Some(2)),
            Ok(Value::Int16(expected))
        );
    }
    let date = legacy("2026-01-01T12:00:00");
    assert_eq!(part("ww", &date, None, Some(1)), Ok(Value::Int16(1)));
    assert_eq!(part("ww", &date, None, Some(2)), Ok(Value::Int16(53)));
    assert_eq!(part("ww", &date, None, Some(3)), Ok(Value::Int16(52)));
    for year in [1, 9999] {
        for (month, day) in [(1, 1), (12, 31)] {
            let date = Value::DateTimeExtended(
                ExtendedDateTime::new(year, month, day, 23, 59, 59, 9_999_999).unwrap(),
            );
            for first in 1..=7 {
                for week in 1..=3 {
                    assert!(matches!(
                        part("ww", &date, Some(first), Some(week)),
                        Ok(Value::Int16(1..=54))
                    ));
                }
            }
        }
    }
}
#[test]
fn policy_omission_null_invalid_intervals_and_types_stay_distinct() {
    let date = legacy("2026-01-01T12:00:00");
    assert_eq!(
        part("ww", &date, Some(0), None),
        Err(EvalError::CalendarPolicyRequired)
    );
    assert_eq!(
        part("ww", &date, None, Some(0)),
        Err(EvalError::CalendarPolicyRequired)
    );
    assert_eq!(part("ww", &date, Some(8), None), Err(EvalError::Domain));
    assert_eq!(
        part("bad", &Value::Null, None, None),
        Err(EvalError::InvalidDateInterval)
    );
    assert_eq!(
        part("d", &Value::Int32(1), None, None),
        Err(EvalError::TypeMismatch)
    );
    assert_eq!(
        date_part(&Value::Text("d".into()), &date, Some(&Value::Null), None),
        Err(EvalError::TypeMismatch)
    );
}
proptest! {
    #![proptest_config(crate::test_support::config())]
    #[test]
    fn weekday_rotation_and_year_day_follow_independent_calendar_offsets(year in 1600_u16..=2400, month in 1_u8..=12, day in 1_u8..=28,first in 1_i32..=7) {
        let value=Value::DateTimeExtended(ExtendedDateTime::new(year,month,day,12,0,0,0).unwrap());
        let civil=conversion::extended_calendar(match value {Value::DateTimeExtended(v)=>v,_=>unreachable!()}).unwrap();
        let ordinal=civil.date().duration_since(jiff::civil::date(1970,1,1)).as_secs()/86400;
        // Unix epoch was Thursday (Sunday-zero offset 4).
        let expected=((ordinal+4-i64::from(first-1)).rem_euclid(7)+1) as i16;
        prop_assert_eq!(part("w",&value,Some(first),None),Ok(Value::Int16(expected)));
        let january=jiff::civil::Date::new(year as i16,1,1).unwrap();
        let expected=civil.date().duration_since(january).as_secs()/86400+1;
        prop_assert_eq!(part("y",&value,None,None),Ok(Value::Int16(expected as i16)));
    }
}

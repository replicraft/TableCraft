use super::*;
use tablecraft_core::values::{Double, LegacyDateTime};

#[test]
fn raw_date_options_round_like_long_before_week_validation() {
    let date = Date::new(2025, 1, 1).unwrap();
    for (number, expected) in [(1.5, 3), (2.5, 3), (3.5, 1), (6.5, 6)] {
        for value in [
            Value::Double(Double::from_float(number)),
            Value::DateTime(LegacyDateTime::from_ole_days(number).unwrap()),
        ] {
            let settings = WeekSettings::parse(Some(&value), None, WeekContext::default()).unwrap();
            assert_eq!(settings.weekday(date), expected);
        }
    }
}

#[test]
fn defaults_system_domain_and_null_remain_separate() {
    let context = WeekContext {
        first_day: Some(FirstDayOfWeek::new(2).unwrap()),
        first_week: Some(FirstWeekOfYear::new(2).unwrap()),
    };
    let zero = Value::DateTime(LegacyDateTime::from_ole_days(-0.5).unwrap());
    assert_eq!(
        WeekSettings::parse(Some(&zero), Some(&zero), context).unwrap(),
        WeekSettings::parse(Some(&Value::Int16(2)), Some(&Value::Int16(2)), context).unwrap()
    );
    assert_ne!(
        WeekSettings::parse(None, None, context).unwrap(),
        WeekSettings::parse(Some(&zero), Some(&zero), context).unwrap()
    );
    assert_eq!(
        WeekSettings::parse(Some(&zero), None, WeekContext::default()),
        Err(EvalError::CalendarPolicyRequired)
    );
    assert_eq!(
        WeekSettings::parse(Some(&Value::Null), None, context),
        Err(EvalError::TypeMismatch)
    );
    assert_eq!(
        WeekSettings::parse(Some(&Value::Int16(8)), None, context),
        Err(EvalError::Domain)
    );
    assert_eq!(
        WeekSettings::parse(Some(&Value::Text("2".into())), None, context),
        Err(EvalError::TypeMismatch)
    );
}

#[test]
fn first_option_domain_is_checked_before_second_conversion() {
    let mut seen = Vec::new();
    let result = WeekSettings::parse_with(
        Some(&Value::Int16(8)),
        Some(&Value::Text("bad".into())),
        WeekContext::default(),
        |value| {
            seen.push(value.clone());
            long(value)
        },
    );
    assert_eq!(result, Err(EvalError::Domain));
    assert_eq!(seen, [Value::Int16(8)]);
}

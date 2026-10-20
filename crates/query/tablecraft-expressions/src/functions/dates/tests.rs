use super::*;
use jiff::civil::{Date, Time};
use proptest::prelude::*;
use tablecraft_core::values::{ExtendedDateTime, LegacyDateTime};

fn components(value: &Value) -> Vec<Value> {
    [
        DateFunction::Year,
        DateFunction::Month,
        DateFunction::Day,
        DateFunction::Hour,
        DateFunction::Minute,
        DateFunction::Second,
    ]
    .map(|function| function.apply(value).unwrap())
    .to_vec()
}
#[test]
fn dates_preserve_null_types_negative_date_rules_and_extended_components() {
    for function in [
        DateFunction::Year,
        DateFunction::Month,
        DateFunction::Day,
        DateFunction::Hour,
        DateFunction::Minute,
        DateFunction::Second,
    ] {
        assert_eq!(function.apply(&Value::Null), Ok(Value::Null));
        assert_eq!(
            function.apply(&Value::Int64(0)),
            Err(EvalError::TypeMismatch)
        );
        assert_eq!(
            function.apply(&Value::Text("2026-01-01".into())),
            Err(EvalError::TypeMismatch)
        );
    }
    assert_eq!(DateFunction::resolve("sEcOnD"), Some(DateFunction::Second));
    assert_eq!(DateFunction::resolve("VBA.Year"), None);
    for (days, expected) in [
        (-1.25, [1899, 12, 29, 6, 0, 0]),
        (-0.25, [1899, 12, 30, 6, 0, 0]),
        (0.25, [1899, 12, 30, 6, 0, 0]),
        (0.0, [1899, 12, 30, 0, 0, 0]),
        (-657434.0, [100, 1, 1, 0, 0, 0]),
        (2958465.0, [9999, 12, 31, 0, 0, 0]),
    ] {
        let value = Value::DateTime(LegacyDateTime::from_ole_days(days).unwrap());
        assert_eq!(components(&value), expected.map(Value::Int16));
    }
    let extended =
        Value::DateTimeExtended(ExtendedDateTime::new(1, 1, 1, 23, 59, 59, 9_999_999).unwrap());
    assert_eq!(
        components(&extended),
        [1, 1, 1, 23, 59, 59].map(Value::Int16)
    );
}
#[test]
fn legacy_whole_second_rounding_carries_without_changing_bits() {
    for (text, expected) in [
        ("2026-03-04T12:34:56", [2026, 3, 4, 12, 34, 56]),
        ("2026-12-31T23:59:59.75", [2027, 1, 1, 0, 0, 0]),
        ("2024-02-28T23:59:59.75", [2024, 2, 29, 0, 0, 0]),
        ("1899-12-29T23:59:59.75", [1899, 12, 30, 0, 0, 0]),
    ] {
        let date = LegacyDateTime::from_calendar(text.parse().unwrap()).unwrap();
        let bits = date.to_bits();
        assert_eq!(
            components(&Value::DateTime(date)),
            expected.map(Value::Int16)
        );
        assert_eq!(date.to_bits(), bits);
    }
    let date = LegacyDateTime::from_calendar("9999-12-31T23:59:59.75".parse().unwrap()).unwrap();
    assert_eq!(
        DateFunction::Year.apply(&Value::DateTime(date)),
        Err(EvalError::Overflow)
    );
}
proptest! {
    #![proptest_config(crate::test_support::config())]
    #[test]
    fn whole_second_components_roundtrip_through_legacy_and_extended(year in 100_i16..=9999, month in 1_i8..=12, day in 1_i8..=28, hour in 0_i8..=23, minute in 0_i8..=59, second in 0_i8..=59) {
        let civil = Date::new(year,month,day).unwrap().to_datetime(Time::new(hour,minute,second,0).unwrap());
        let expected = [year,month as i16,day as i16,hour as i16,minute as i16,second as i16].map(Value::Int16);
        let legacy = LegacyDateTime::from_calendar(civil).unwrap();
        let extended = ExtendedDateTime::new(year as u16,month as u8,day as u8,hour as u8,minute as u8,second as u8,0).unwrap();
        prop_assert_eq!(components(&Value::DateTime(legacy)),expected.clone());
        prop_assert_eq!(components(&Value::DateTimeExtended(extended)),expected);
    }
}

use super::*;
use proptest::prelude::*;
use tablecraft_core::values::{Currency, Decimal, Double, ExtendedDateTime};
fn legacy(text: &str) -> Value {
    Value::DateTime(LegacyDateTime::from_calendar(text.parse().unwrap()).unwrap())
}
fn add(interval: &str, number: Value, value: &Value) -> Result<Value, EvalError> {
    date_add(&Value::Text(interval.into()), &number, value)
}
#[test]
fn all_interval_spellings_follow_calendar_rules_and_month_end_clamping() {
    for (interval, count, start, end) in [
        ("YYYY", 1, "2024-02-29T12:00:00", "2025-02-28T12:00:00"),
        ("q", 1, "2024-01-31T12:00:00", "2024-04-30T12:00:00"),
        ("m", 1, "2024-01-31T12:00:00", "2024-02-29T12:00:00"),
        ("m", -1, "2023-03-31T12:00:00", "2023-02-28T12:00:00"),
        ("y", 1, "2024-02-28T12:00:00", "2024-02-29T12:00:00"),
        ("d", -1, "1899-12-30T06:00:00", "1899-12-29T06:00:00"),
        ("w", 1, "2026-01-02T12:00:00", "2026-01-03T12:00:00"),
        ("ww", 1, "2026-01-02T12:00:00", "2026-01-09T12:00:00"),
        ("h", 1, "2024-12-31T23:00:00", "2025-01-01T00:00:00"),
        ("n", 1, "2024-12-31T23:59:00", "2025-01-01T00:00:00"),
        ("s", 1, "2024-12-31T23:59:59", "2025-01-01T00:00:00"),
    ] {
        assert_eq!(
            add(interval, Value::Int32(count), &legacy(start)),
            Ok(legacy(end))
        );
    }
}
#[test]
fn counts_reuse_exact_half_even_coercion_and_errors_remain_explicit() {
    let start = legacy("2026-01-01T12:00:00");
    for count in [
        Value::Decimal(Decimal::new(25, 1).unwrap()),
        Value::Currency(Currency::from_scaled(25000)),
        Value::Double(Double::from_float(2.5)),
    ] {
        assert_eq!(add("d", count, &start), Ok(legacy("2026-01-03T12:00:00")));
    }
    assert_eq!(
        add("d", Value::Double(Double::from_float(-1.5)), &start),
        Ok(legacy("2025-12-30T12:00:00"))
    );
    assert_eq!(
        add("d", Value::Boolean(true), &start),
        Ok(legacy("2025-12-31T12:00:00"))
    );
    assert_eq!(add("n", Value::Int32(1), &Value::Null), Ok(Value::Null));
    assert_eq!(add("n", Value::Null, &start), Err(EvalError::TypeMismatch));
    assert_eq!(
        add("milliseconds", Value::Int32(1), &start),
        Err(EvalError::InvalidDateInterval)
    );
    assert_eq!(
        add(" d", Value::Int32(1), &Value::Null),
        Err(EvalError::InvalidDateInterval)
    );
    assert_eq!(
        add("d", Value::Int64(i64::MAX), &start),
        Err(EvalError::Overflow)
    );
    assert_eq!(
        add("s", Value::Double(Double::from_float(f64::NAN)), &start),
        Err(EvalError::NonFinite)
    );
    assert_eq!(
        add("d", Value::Int32(-1), &legacy("0100-01-01T00:00:00")),
        Err(EvalError::Domain)
    );
    assert_eq!(
        add("s", Value::Int32(1), &legacy("9999-12-31T23:59:59")),
        Err(EvalError::Overflow)
    );
    let before = legacy("2026-01-01T00:00:00.75");
    assert_eq!(
        add("s", Value::Int32(0), &before),
        Ok(legacy("2026-01-01T00:00:01"))
    );
}
#[test]
fn extended_dates_preserve_carrier_fraction_and_separate_year_envelope() {
    let value = Value::DateTimeExtended(ExtendedDateTime::new(1, 1, 1, 0, 0, 0, 1).unwrap());
    assert_eq!(
        add("s", Value::Int32(1), &value),
        Ok(Value::DateTimeExtended(
            ExtendedDateTime::new(1, 1, 1, 0, 0, 1, 1).unwrap()
        ))
    );
    assert_eq!(add("s", Value::Int32(-1), &value), Err(EvalError::Overflow));
    let value = Value::DateTimeExtended(
        ExtendedDateTime::new(9999, 12, 31, 23, 59, 59, 9_999_999).unwrap(),
    );
    assert_eq!(add("s", Value::Int32(0), &value), Ok(value.clone()));
    assert_eq!(add("s", Value::Int32(1), &value), Err(EvalError::Overflow));
}
proptest! {
    #![proptest_config(crate::test_support::config())]
    #[test]
    fn extended_second_additions_are_reversible_without_fraction_loss(seconds in -1_000_000_i32..=1_000_000, ticks in 0_u32..10_000_000) {
        let original = Value::DateTimeExtended(
            ExtendedDateTime::new(2026, 1, 15, 12, 34, 56, ticks).unwrap(),
        );
        let after = add("s", Value::Int32(seconds), &original).unwrap();
        prop_assert_eq!(add("s", Value::Int32(-seconds), &after), Ok(original));
    }
}

#[test]
fn legacy_date_counts_use_raw_serial_long_conversion() {
    let start = legacy("2025-01-01T12:00:00");
    for (serial, expected) in [(2.5, "2025-01-03T12:00:00"), (-1.5, "2024-12-30T12:00:00")] {
        let count = Value::DateTime(LegacyDateTime::from_ole_days(serial).unwrap());
        assert_eq!(add("d", count, &start), Ok(legacy(expected)));
    }
    assert_eq!(
        add("d", Value::Text("2.5".into()), &start),
        Err(EvalError::TypeMismatch)
    );
}

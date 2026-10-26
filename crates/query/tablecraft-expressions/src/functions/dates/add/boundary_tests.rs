use super::*;
use tablecraft_core::values::ExtendedDateTime;

fn legacy(text: &str) -> Value {
    Value::DateTime(LegacyDateTime::from_calendar(text.parse().unwrap()).unwrap())
}

fn add(interval: &str, count: i32, source: &Value) -> Result<Value, EvalError> {
    date_add(&Value::Text(interval.into()), &Value::Int32(count), source)
}

#[test]
fn every_legacy_interval_underflow_uses_error_five() {
    let first = legacy("0100-01-01T00:00:00");
    for interval in ["yyyy", "q", "m", "y", "d", "w", "ww", "h", "n", "s"] {
        assert_eq!(
            add(interval, -1, &first),
            Err(EvalError::Domain),
            "{interval}"
        );
        assert_eq!(add(interval, 0, &first), Ok(first.clone()), "{interval}");
        assert_eq!(
            add(interval, i32::MIN, &first),
            Err(EvalError::Domain),
            "{interval}"
        );
    }
}

#[test]
fn valid_lower_endpoints_upper_failures_and_extended_years_remain_distinct() {
    assert_eq!(
        add("d", -1, &legacy("0100-01-02T00:00:00")),
        Ok(legacy("0100-01-01T00:00:00"))
    );
    assert_eq!(
        add("s", 1, &legacy("9999-12-31T23:59:59")),
        Err(EvalError::Overflow)
    );
    assert_eq!(
        add("yyyy", i32::MAX, &legacy("0100-01-01T00:00:00")),
        Err(EvalError::Overflow)
    );

    let extended = Value::DateTimeExtended(ExtendedDateTime::new(100, 1, 1, 0, 0, 0, 1).unwrap());
    let previous =
        Value::DateTimeExtended(ExtendedDateTime::new(99, 12, 31, 23, 59, 59, 1).unwrap());
    assert_eq!(add("s", -1, &extended), Ok(previous));
    assert_eq!(add("yyyy", i32::MIN, &extended), Err(EvalError::Overflow));
}

#[test]
fn calendar_classification_does_not_replace_control_or_null_admission() {
    let source = legacy("0100-01-01T00:00:00");
    assert_eq!(add("bad", -1, &source), Err(EvalError::InvalidDateInterval));
    assert_eq!(add("s", i32::MIN, &Value::Null), Ok(Value::Null));
    assert_eq!(
        date_add(
            &Value::Text("s".into()),
            &Value::Int64(-2_147_483_649),
            &Value::Null
        ),
        Err(EvalError::Overflow)
    );
    assert_eq!(
        add("s", -1, &Value::Text("bad".into())),
        Err(EvalError::TypeMismatch)
    );
}

use crate::{
    EvalError,
    functions::text::{TextFunction as F, TextResult, apply},
};
use tablecraft_core::values::{Currency, Decimal, Double, LegacyDateTime, Value};

#[test]
fn fractional_numeric_date_and_boolean_counts_share_checked_long_rounding() {
    for count in [
        Value::Double(Double::from_float(2.5)),
        Value::Currency(Currency::from_scaled(25000)),
        Value::Decimal(Decimal::new(25, 1).unwrap()),
        Value::DateTime(LegacyDateTime::from_ole_days(2.5).unwrap()),
    ] {
        for (function, expected) in [(F::Left, "ab"), (F::Right, "cd"), (F::Mid, "bcd")] {
            let arguments = [Value::Text("abcd".into()), count.clone()];
            assert_eq!(
                apply(function, &arguments, || Ok(())).unwrap(),
                TextResult::Text(expected)
            );
        }
    }
    let arguments = [
        Value::Text("abcd".into()),
        Value::Double(Double::from_float(1.5)),
        Value::Decimal(Decimal::new(25, 1).unwrap()),
    ];
    assert_eq!(
        apply(F::Mid, &arguments, || Ok(())).unwrap(),
        TextResult::Text("bc")
    );
    let arguments = [Value::Text("abc".into()), Value::Boolean(false)];
    assert_eq!(
        apply(F::Left, &arguments, || Ok(())).unwrap(),
        TextResult::Text("")
    );
}

#[test]
fn domain_overflow_null_and_surrogate_checks_follow_count_conversion() {
    for (count, expected) in [
        (Value::Double(Double::from_float(-1.5)), EvalError::Domain),
        (
            Value::Double(Double::from_float(2147483647.5)),
            EvalError::Overflow,
        ),
        (
            Value::Double(Double::from_float(f64::INFINITY)),
            EvalError::NonFinite,
        ),
        (Value::Text("2.5".into()), EvalError::TypeMismatch),
        (Value::Null, EvalError::TypeMismatch),
    ] {
        let arguments = [Value::Text("abc".into()), count];
        assert_eq!(apply(F::Left, &arguments, || Ok(())), Err(expected));
    }
    let arguments = [
        Value::Text("é🦀中".into()),
        Value::Double(Double::from_float(2.5)),
    ];
    assert_eq!(
        apply(F::Left, &arguments, || Ok(())),
        Err(EvalError::UnrepresentableText)
    );
    let arguments = [Value::Null, Value::Text("bad".into())];
    assert_eq!(apply(F::Left, &arguments, || Ok(())), Ok(TextResult::Null));
    let arguments = [
        Value::Text("abc".into()),
        Value::Int16(0),
        Value::Double(Double::from_float(f64::INFINITY)),
    ];
    assert_eq!(apply(F::Mid, &arguments, || Ok(())), Err(EvalError::Domain));
}

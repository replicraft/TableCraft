use super::*;
use crate::functions::text::format::tests::render;
use tablecraft_core::values::{Currency, Decimal, Double, LegacyDateTime, Single};

const MASK: &str = r#"yyyy;"negative";"epoch";"missing""#;

#[test]
fn positive_numeric_serials_render_dates_without_narrowing_carriers() {
    for value in [
        Value::Byte(2),
        Value::Int16(2),
        Value::Int32(2),
        Value::Int64(2),
        Value::Single(Single::from_float(2.0)),
        Value::Double(Double::from_float(2.0)),
        Value::Currency(Currency::from_scaled(20_000)),
        Value::Decimal(Decimal::new(200, 2).unwrap()),
    ] {
        assert_eq!(render(value, MASK, None), Ok("1900".into()));
    }
}

#[test]
fn nonpositive_numeric_sections_and_null_keep_literal_rendering() {
    for value in [
        Value::Boolean(true),
        Value::Int16(-2),
        Value::Double(Double::from_float(-2.5)),
        Value::Currency(Currency::from_scaled(-25_000)),
        Value::Decimal(Decimal::new(-25, 1).unwrap()),
    ] {
        assert_eq!(render(value, MASK, None), Ok("negative".into()));
    }
    for value in [
        Value::Boolean(false),
        Value::Int32(0),
        Value::Double(Double::from_float(-0.0)),
    ] {
        assert_eq!(render(value, MASK, None), Ok("epoch".into()));
    }
    assert_eq!(render(Value::Null, MASK, None), Ok("missing".into()));
    assert_eq!(
        render(Value::Int32(-1), r#"yyyy;"a;b"\!;"zero""#, None),
        Ok("a;b!".into())
    );
}

#[test]
fn unresolved_date_sections_and_nonfinite_values_remain_explicit() {
    let date = Value::DateTime(LegacyDateTime::from_ole_days(2.0).unwrap());
    for (value, mask) in [
        (date, MASK),
        (Value::Text("1900-01-01".into()), MASK),
        (Value::Int32(-2), "yyyy;yyyy"),
        (Value::Int32(0), "yyyy;yyyy"),
        (Value::Int32(-2), "yyyy;;\"zero\""),
        (Value::Int32(-2), "yyyy;0;\"zero\""),
    ] {
        assert_eq!(
            render(value, mask, None),
            Err(EvalError::UnsupportedExpression)
        );
    }
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(
            render(Value::Double(Double::from_float(value)), MASK, None),
            Err(EvalError::NonFinite)
        );
    }
}

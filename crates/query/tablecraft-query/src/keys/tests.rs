use super::*;
use tablecraft_core::values::{Currency, Decimal, Double, Single};

#[test]
fn stored_ordering_retains_exact_mixed_numeric_keys() {
    let binary = Value::Double(Double::from_float(9_007_199_254_740_992.0));
    let integral = Value::Int64(9_007_199_254_740_993);
    let decimal = Value::Decimal(Decimal::new(9_007_199_254_740_993, 0).unwrap());
    assert_eq!(
        compare_stored_keys(&binary, &integral, None),
        Ok(Ordering::Less)
    );
    assert_eq!(
        compare_stored_keys(&integral, &decimal, None),
        Ok(Ordering::Equal)
    );
    assert_eq!(
        compare_stored_keys(&decimal, &binary, None),
        Ok(Ordering::Greater)
    );

    let single = Value::Single(Single::from_float(16_777_216.0));
    let double = Value::Double(Double::from_float(16_777_217.0));
    assert_eq!(
        compare_stored_keys(&single, &double, None),
        Ok(Ordering::Less)
    );
    assert_eq!(
        compare_stored_keys(
            &Value::Currency(Currency::from_scaled(1)),
            &Value::Int32(0),
            None
        ),
        Ok(Ordering::Greater)
    );
}

#[test]
fn stored_key_admission_and_nulls_never_hide_invalid_or_coerced_operands() {
    let invalid = Value::Double(Double::from_float(f64::NAN));
    assert_eq!(
        validate_stored_key(&invalid),
        Err(EvalError::NonFinite.into())
    );
    assert_eq!(
        compare_stored_keys(&Value::Null, &invalid, None),
        Err(EvalError::NonFinite.into())
    );
    assert_eq!(
        compare_stored_keys(&Value::Null, &Value::Int32(0), None),
        Ok(Ordering::Less)
    );
    assert_eq!(
        compare_stored_keys(&Value::Text("0".into()), &Value::Int32(0), None),
        Err(EvalError::TypeMismatch.into())
    );
    assert_eq!(
        compare_stored_keys(&Value::Text("a".into()), &Value::Text("b".into()), None),
        Err(EvalError::CollationRequired.into())
    );
}

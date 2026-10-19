use super::*;
use tablecraft_core::values::{Currency, Decimal, Double, Single};

#[test]
fn null_testing_does_not_confuse_zero_or_empty_values_and_lookup_is_closed() {
    assert!(is_null(&Value::Null));
    for value in [
        Value::Int32(0),
        Value::Boolean(false),
        Value::Text(String::new()),
        Value::Binary(Vec::new()),
    ] {
        assert!(!is_null(&value));
    }
    assert_eq!(Builtin::resolve("iIf"), Some(Builtin::IIf));
    for name in ["VBA.IIf", "IsNull$", "Run", "IIf "] {
        assert_eq!(Builtin::resolve(name), None);
    }
}
#[test]
fn conditional_truth_keeps_full_width_numeric_precision_and_branch_carriers() {
    let false_conditions = [
        Value::Null,
        Value::Boolean(false),
        Value::Int64(0),
        Value::Single(Single::from_float(-0.0)),
        Value::Double(Double::from_float(0.0)),
        Value::Currency(Currency::from_scaled(0)),
        Value::Decimal(Decimal::new(0, 28).unwrap()),
    ];
    for condition in false_conditions {
        assert_eq!(
            iif(&condition, Value::Int16(7), Value::Text("no".into())),
            Ok(Value::Text("no".into()))
        );
    }
    let true_conditions = [
        Value::Boolean(true),
        Value::Byte(255),
        Value::Int16(i16::MIN),
        Value::Int32(i32::MAX),
        Value::Int64(i64::MAX),
        Value::Int64(i64::MIN),
        Value::Single(Single::from_float(f32::MIN_POSITIVE)),
        Value::Double(Double::from_float(f64::from_bits(1))),
        Value::Currency(Currency::from_scaled(-1)),
        Value::Decimal(Decimal::new(1, 28).unwrap()),
    ];
    for condition in true_conditions {
        assert_eq!(
            iif(&condition, Value::Binary(vec![0, 255]), Value::Null),
            Ok(Value::Binary(vec![0, 255]))
        );
    }
    for condition in [Value::Text("True".into()), Value::Binary(vec![])] {
        assert_eq!(
            iif(&condition, Value::Null, Value::Null),
            Err(EvalError::TypeMismatch)
        );
    }
    assert_eq!(
        iif(
            &Value::Double(Double::from_float(f64::NAN)),
            Value::Null,
            Value::Null
        ),
        Err(EvalError::NonFinite)
    );
}

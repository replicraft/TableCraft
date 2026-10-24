use super::*;
use crate::numeric::{
    Arithmetic, ArithmeticDeclarations, arithmetic, declared_numeric_arithmetic,
    promoted_arithmetic,
};
use tablecraft_core::values::{Double, ExtendedDateTime, Guid, LegacyDateTime, Single};

const OPERATIONS: [Arithmetic; 7] = [
    Arithmetic::Add,
    Arithmetic::Subtract,
    Arithmetic::Multiply,
    Arithmetic::Divide,
    Arithmetic::IntegerDivide,
    Arithmetic::Modulo,
    Arithmetic::Power,
];

#[test]
fn unsupported_carriers_cannot_be_hidden_by_null_in_checked_or_promoted_arithmetic() {
    for value in [
        Value::Binary(vec![0, 1, 255]),
        Value::Guid(Guid::from_bytes([1; 16])),
        Value::DateTimeExtended(ExtendedDateTime::new(2026, 1, 1, 0, 0, 0, 1).unwrap()),
        Value::Text("1".into()),
    ] {
        for operator in OPERATIONS {
            for (left, right) in [(&Value::Null, &value), (&value, &Value::Null)] {
                assert_eq!(
                    arithmetic(operator, left, right),
                    Err(EvalError::TypeMismatch)
                );
                assert_eq!(
                    promoted_arithmetic(operator, left, right),
                    Err(EvalError::TypeMismatch)
                );
                assert_eq!(
                    declared_numeric_arithmetic(
                        operator,
                        left,
                        right,
                        ArithmeticDeclarations::default()
                    ),
                    Err(EvalError::TypeMismatch)
                );
            }
        }
    }

    let legacy = Value::DateTime(LegacyDateTime::from_ole_days(-0.25).unwrap());
    for operator in OPERATIONS {
        assert_eq!(
            arithmetic(operator, &Value::Null, &legacy),
            Err(EvalError::TypeMismatch)
        );
        assert_eq!(
            promoted_arithmetic(operator, &Value::Null, &legacy),
            Ok(Value::Null)
        );
    }
}

#[test]
fn nonfinite_values_do_not_disappear_beside_null_and_finite_values_do_not_convert() {
    for value in [
        Value::Single(Single::from_bits(0x7fc0_0001)),
        Value::Single(Single::from_bits(0x7f80_0000)),
        Value::Double(Double::from_bits(0x7ff8_0000_0000_0001)),
        Value::Double(Double::from_bits(0xfff0_0000_0000_0000)),
    ] {
        for operator in OPERATIONS {
            for (left, right) in [(&Value::Null, &value), (&value, &Value::Null)] {
                assert_eq!(arithmetic(operator, left, right), Err(EvalError::NonFinite));
                assert_eq!(
                    promoted_arithmetic(operator, left, right),
                    Err(EvalError::NonFinite)
                );
                assert_eq!(
                    declared_numeric_arithmetic(
                        operator,
                        left,
                        right,
                        ArithmeticDeclarations::default()
                    ),
                    Err(EvalError::NonFinite)
                );
            }
        }
    }

    for value in [
        Value::Int64(i64::MAX),
        Value::Double(Double::from_float(f64::MAX)),
    ] {
        for operator in OPERATIONS {
            assert_eq!(arithmetic(operator, &Value::Null, &value), Ok(Value::Null));
            assert_eq!(
                promoted_arithmetic(operator, &Value::Null, &value),
                Ok(Value::Null)
            );
        }
    }
}

#[test]
fn materialized_admission_checks_operands_in_written_order_without_coercion() {
    let nonfinite = Value::Double(Double::from_bits(0x7ff0_0000_0000_0000));
    let binary = Value::Binary(Vec::new());
    assert_eq!(
        promoted_arithmetic(Arithmetic::Add, &nonfinite, &binary),
        Err(EvalError::NonFinite)
    );
    assert_eq!(
        promoted_arithmetic(Arithmetic::Add, &binary, &nonfinite),
        Err(EvalError::TypeMismatch)
    );
}

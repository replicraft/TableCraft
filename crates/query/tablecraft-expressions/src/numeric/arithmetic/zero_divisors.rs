use super::*;
use crate::numeric::{
    ArithmeticDeclarations, OperandDeclaration, declared_numeric_arithmetic, promoted_arithmetic,
};
use tablecraft_core::values::{LegacyDateTime, Single};

fn zeros() -> Vec<Value> {
    vec![
        Value::Boolean(false),
        Value::Byte(0),
        Value::Int16(0),
        Value::Int32(0),
        Value::Int64(0),
        Value::Single(Single::from_float(0.0)),
        Value::Single(Single::from_float(-0.0)),
        Value::Double(Double::from_float(0.0)),
        Value::Double(Double::from_float(-0.0)),
        Value::Currency(Currency::from_scaled(0)),
        Value::DateTime(LegacyDateTime::from_ole_days(0.0).unwrap()),
    ]
}

#[test]
fn floating_selected_zero_over_zero_overflows_independently_of_declaration() {
    for left in zeros() {
        for right in zeros() {
            assert_eq!(
                promoted_arithmetic(Arithmetic::Divide, &left, &right),
                Err(EvalError::Overflow),
                "{left:?} / {right:?}"
            );
            if !matches!(left, Value::Currency(_) | Value::DateTime(_))
                && !matches!(right, Value::Currency(_) | Value::DateTime(_))
            {
                assert_eq!(
                    arithmetic(Arithmetic::Divide, &left, &right),
                    Err(EvalError::Overflow)
                );
            }
            if matches!(left, Value::DateTime(_)) || matches!(right, Value::DateTime(_)) {
                continue;
            }
            for left_declaration in [OperandDeclaration::Scalar, OperandDeclaration::Variant] {
                for right_declaration in [OperandDeclaration::Scalar, OperandDeclaration::Variant] {
                    assert_eq!(
                        declared_numeric_arithmetic(
                            Arithmetic::Divide,
                            &left,
                            &right,
                            ArithmeticDeclarations {
                                left: left_declaration,
                                right: right_declaration,
                            }
                        ),
                        Err(EvalError::Overflow)
                    );
                }
            }
        }
    }
}

#[test]
fn decimal_and_rounded_division_keep_division_by_zero() {
    for zero in zeros() {
        for decimal in [Decimal::new(0, 0).unwrap(), Decimal::new(0, 28).unwrap()] {
            for (left, right) in [
                (&zero, &Value::Decimal(decimal)),
                (&Value::Decimal(decimal), &zero),
            ] {
                assert_eq!(
                    promoted_arithmetic(Arithmetic::Divide, left, right),
                    Err(EvalError::DivisionByZero)
                );
            }
        }
        for operator in [Arithmetic::IntegerDivide, Arithmetic::Modulo] {
            assert_eq!(
                promoted_arithmetic(operator, &zero, &Value::Double(Double::from_float(0.5))),
                Err(EvalError::DivisionByZero)
            );
        }
    }
}

#[test]
fn nonzero_numerators_and_signed_zero_divisors_keep_their_distinct_error() {
    for left in [
        Value::Int32(1),
        Value::Int32(-1),
        Value::Single(Single::from_bits(1)),
        Value::Double(Double::from_bits(1)),
        Value::Currency(Currency::from_scaled(1)),
        Value::DateTime(LegacyDateTime::from_ole_days(-0.25).unwrap()),
    ] {
        for right in zeros() {
            assert_eq!(
                promoted_arithmetic(Arithmetic::Divide, &left, &right),
                Err(EvalError::DivisionByZero),
                "{left:?} / {right:?}"
            );
        }
    }
    for zero in zeros() {
        assert_eq!(
            promoted_arithmetic(Arithmetic::Divide, &Value::Null, &zero),
            Ok(Value::Null)
        );
        assert_eq!(
            promoted_arithmetic(Arithmetic::Divide, &zero, &Value::Null),
            Ok(Value::Null)
        );
    }
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(
            promoted_arithmetic(
                Arithmetic::Divide,
                &Value::Double(Double::from_float(value)),
                &Value::Int32(0)
            ),
            Err(EvalError::NonFinite)
        );
    }
}

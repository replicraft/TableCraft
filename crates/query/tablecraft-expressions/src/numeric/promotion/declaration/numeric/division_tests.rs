use super::*;
use tablecraft_core::values::{Currency, Decimal, Double, LegacyDateTime, Single};

fn decimal(value: i128) -> Value {
    Value::Decimal(Decimal::new(value, 0).unwrap())
}

fn double(value: f64) -> Value {
    Value::Double(Double::from_float(value))
}

#[test]
fn mixed_decimal_division_selects_double_for_every_category_in_both_orders() {
    let peers = [
        double(3.0),
        Value::Currency(Currency::from_scaled(30_000)),
        Value::DateTime(LegacyDateTime::from_ole_days(3.0).unwrap()),
    ];
    for peer in peers {
        for (left, right, expected) in [
            (decimal(1), peer.clone(), double(1.0 / 3.0)),
            (peer, decimal(1), double(3.0)),
        ] {
            assert!(uses_declared_double_division(
                Arithmetic::Divide,
                &left,
                &right
            ));
            assert!(matches!(
                promoted_arithmetic(Arithmetic::Divide, &left, &right),
                Ok(Value::Decimal(_))
            ));
            assert_eq!(
                arithmetic_agreement(Arithmetic::Divide, &left, &right),
                Ok(Some(expected.clone()))
            );
            for a in [OperandDeclaration::Scalar, OperandDeclaration::Variant] {
                for b in [OperandDeclaration::Scalar, OperandDeclaration::Variant] {
                    assert_eq!(
                        declared_numeric_arithmetic(
                            Arithmetic::Divide,
                            &left,
                            &right,
                            ArithmeticDeclarations { left: a, right: b }
                        ),
                        Ok(expected.clone())
                    );
                }
            }
        }
    }
}

#[test]
fn small_and_huge_double_peers_do_not_pass_through_decimal() {
    for peer in [1e-100, 1e100] {
        for (left, right, expected) in [
            (decimal(1), double(peer), 1.0 / peer),
            (double(peer), decimal(1), peer),
        ] {
            assert_eq!(
                arithmetic_agreement(Arithmetic::Divide, &left, &right),
                Ok(Some(double(expected)))
            );
        }
    }
}

#[test]
fn other_decimal_peers_and_operators_keep_existing_selection() {
    for peer in [
        Value::Single(Single::from_float(3.0)),
        Value::Int16(3),
        decimal(3),
    ] {
        for (left, right) in [(decimal(1), peer.clone()), (peer, decimal(1))] {
            assert!(!uses_declared_double_division(
                Arithmetic::Divide,
                &left,
                &right
            ));
            assert!(matches!(
                arithmetic_agreement(Arithmetic::Divide, &left, &right),
                Ok(Some(Value::Decimal(_)))
            ));
        }
    }
    for operation in [
        Arithmetic::Add,
        Arithmetic::Subtract,
        Arithmetic::Multiply,
        Arithmetic::Power,
    ] {
        assert!(!uses_declared_double_division(
            operation,
            &decimal(1),
            &double(2.0)
        ));
    }
}

#[test]
fn double_zero_class_and_original_decimal_zero_class_remain_distinct() {
    assert_eq!(
        arithmetic_agreement(Arithmetic::Divide, &decimal(0), &double(0.0)),
        Err(EvalError::Overflow)
    );
    assert_eq!(
        arithmetic_agreement(Arithmetic::Divide, &decimal(1), &double(-0.0)),
        Err(EvalError::DivisionByZero)
    );
    assert_eq!(
        arithmetic_agreement(Arithmetic::Divide, &decimal(0), &decimal(0)),
        Err(EvalError::DivisionByZero)
    );
    assert_eq!(
        arithmetic_agreement(Arithmetic::Divide, &decimal(0), &double(-1.0)),
        Ok(Some(double(-0.0)))
    );
    assert_eq!(
        arithmetic_agreement(Arithmetic::Divide, &Value::Null, &decimal(1)),
        Ok(Some(Value::Null))
    );
}

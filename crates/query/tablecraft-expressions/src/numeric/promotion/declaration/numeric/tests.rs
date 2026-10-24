use super::*;
use tablecraft_core::values::{Currency, Decimal, Double, LegacyDateTime, Single};

fn single(value: f32) -> Value {
    Value::Single(Single::from_float(value))
}

fn context(left: bool, right: bool) -> ArithmeticDeclarations {
    let category = |variant| {
        if variant {
            OperandDeclaration::Variant
        } else {
            OperandDeclaration::Scalar
        }
    };
    ArithmeticDeclarations {
        left: category(left),
        right: category(right),
    }
}

#[test]
fn fixed_long_long_results_do_not_require_a_scalar_variant_width_decision() {
    for left in [i64::MIN, -32768, -1, 0, 1, 32767, i64::MAX] {
        for right in [i64::MIN, -1, 0, 1, i64::MAX] {
            for operator in [
                Arithmetic::Add,
                Arithmetic::Subtract,
                Arithmetic::Multiply,
                Arithmetic::IntegerDivide,
                Arithmetic::Modulo,
            ] {
                let left = Value::Int64(left);
                let right = Value::Int64(right);
                assert!(!uses_declared_overflow(operator, &left, &right));
                let expected = promoted_arithmetic(operator, &left, &right);

                for a in [false, true] {
                    for b in [false, true] {
                        assert_eq!(
                            declared_numeric_arithmetic(operator, &left, &right, context(a, b)),
                            expected
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn remainders_and_long_long_quotients_keep_conversion_failures_without_category_dispatch() {
    for left in [0, 1, 255] {
        for right in [0, 1, 2, 255] {
            let left = Value::Byte(left);
            let right = Value::Byte(right);
            assert!(!uses_declared_overflow(
                Arithmetic::IntegerDivide,
                &left,
                &right
            ));
            let expected = promoted_arithmetic(Arithmetic::IntegerDivide, &left, &right);
            for a in [false, true] {
                for b in [false, true] {
                    assert_eq!(
                        declared_numeric_arithmetic(
                            Arithmetic::IntegerDivide,
                            &left,
                            &right,
                            context(a, b)
                        ),
                        expected
                    );
                }
            }
        }
    }

    for (left, right) in [
        (Value::Int16(i16::MIN), Value::Int16(-1)),
        (Value::Int32(i32::MIN), Value::Int32(-1)),
        (Value::Int64(i64::MIN), Value::Int64(-1)),
        (single(7.5), Value::Double(Double::from_float(2.5))),
        (
            Value::Currency(Currency::from_scaled(75_000)),
            Value::Byte(2),
        ),
        (
            Value::Decimal(Decimal::new(75, 1).unwrap()),
            Value::Int16(2),
        ),
    ] {
        assert!(!uses_declared_overflow(Arithmetic::Modulo, &left, &right));
        let expected = promoted_arithmetic(Arithmetic::Modulo, &left, &right);
        for a in [false, true] {
            for b in [false, true] {
                assert_eq!(
                    declared_numeric_arithmetic(Arithmetic::Modulo, &left, &right, context(a, b)),
                    expected
                );
            }
        }
    }

    let left = Value::Int64(7);
    for right in [
        single(2.5),
        Value::Decimal(Decimal::new(25, 1).unwrap()),
        Value::Double(Double::from_float(0.5)),
        Value::Double(Double::from_float(f64::MAX)),
    ] {
        assert!(!uses_declared_overflow(
            Arithmetic::IntegerDivide,
            &left,
            &right
        ));
        let expected = promoted_arithmetic(Arithmetic::IntegerDivide, &left, &right);
        for a in [false, true] {
            for b in [false, true] {
                assert_eq!(
                    declared_numeric_arithmetic(
                        Arithmetic::IntegerDivide,
                        &left,
                        &right,
                        context(a, b)
                    ),
                    expected
                );
            }
        }
    }
}

#[test]
fn single_overflow_preserves_original_variant_participation_in_each_operator() {
    for (operation, left, right, expected) in [
        (
            Arithmetic::Add,
            single(f32::MAX),
            single(f32::MAX),
            f64::from(f32::MAX) * 2.0,
        ),
        (
            Arithmetic::Subtract,
            single(-f32::MAX),
            single(f32::MAX),
            -f64::from(f32::MAX) * 2.0,
        ),
        (
            Arithmetic::Multiply,
            single(f32::MAX),
            Value::Int16(2),
            f64::from(f32::MAX) * 2.0,
        ),
        (
            Arithmetic::Divide,
            single(f32::MAX),
            single(0.5),
            f64::from(f32::MAX) * 2.0,
        ),
    ] {
        for left_variant in [false, true] {
            for right_variant in [false, true] {
                assert_eq!(
                    declared_numeric_arithmetic(
                        operation,
                        &left,
                        &right,
                        context(left_variant, right_variant)
                    ),
                    if left_variant || right_variant {
                        Ok(Value::Double(Double::from_float(expected)))
                    } else {
                        Err(EvalError::Overflow)
                    }
                );
            }
        }
    }
}

#[test]
fn effective_width_dispatch_signed_zero_and_underflow_are_not_value_guesses() {
    let max = single(f32::MAX);
    assert!(uses_declared_overflow(
        Arithmetic::Multiply,
        &max,
        &Value::Int16(2)
    ));
    assert!(!uses_declared_overflow(
        Arithmetic::Multiply,
        &max,
        &Value::Int32(2)
    ));
    assert!(uses_declared_overflow(
        Arithmetic::IntegerDivide,
        &single(7.5),
        &single(2.5)
    ));
    assert!(!uses_declared_overflow(
        Arithmetic::Power,
        &max,
        &single(2.0)
    ));
    assert!(!uses_declared_overflow(
        Arithmetic::Divide,
        &Value::Int16(3),
        &Value::Int16(2)
    ));
    assert!(!uses_declared_overflow(Arithmetic::Add, &Value::Null, &max));
    for declarations in [
        context(false, false),
        context(false, true),
        context(true, false),
        context(true, true),
    ] {
        assert_eq!(
            declared_numeric_arithmetic(Arithmetic::Multiply, &max, &Value::Int32(2), declarations),
            Ok(Value::Double(Double::from_float(f64::from(f32::MAX) * 2.0)))
        );
        assert_eq!(
            declared_numeric_arithmetic(Arithmetic::Divide, &max, &single(2.0), declarations),
            Ok(single(f32::MAX / 2.0))
        );
        assert_eq!(
            declared_numeric_arithmetic(
                Arithmetic::Multiply,
                &single(-0.0),
                &Value::Byte(1),
                declarations
            ),
            Ok(single(-0.0))
        );
        assert_eq!(
            declared_numeric_arithmetic(
                Arithmetic::Divide,
                &single(f32::from_bits(1)),
                &single(2.0),
                declarations
            ),
            Ok(single(0.0))
        );
        assert_eq!(
            declared_numeric_arithmetic(
                Arithmetic::IntegerDivide,
                &single(7.5),
                &single(2.5),
                declarations
            ),
            Ok(Value::Int32(4))
        );
    }
}

#[test]
fn family_validation_and_existing_fixed_point_double_and_integral_rules_remain_checked() {
    for declarations in [
        context(false, false),
        context(false, true),
        context(true, false),
        context(true, true),
    ] {
        assert_eq!(
            declared_numeric_arithmetic(
                Arithmetic::Add,
                &Value::Null,
                &Value::Text("1".into()),
                declarations
            ),
            Err(EvalError::TypeMismatch)
        );
        assert_eq!(
            declared_numeric_arithmetic(
                Arithmetic::Multiply,
                &Value::Null,
                &single(1.0),
                declarations
            ),
            Ok(Value::Null)
        );
        assert_eq!(
            declared_numeric_arithmetic(
                Arithmetic::Add,
                &single(f32::INFINITY),
                &Value::Byte(1),
                declarations
            ),
            Err(EvalError::NonFinite)
        );
        assert_eq!(
            declared_numeric_arithmetic(
                Arithmetic::Divide,
                &single(1.0),
                &single(0.0),
                declarations
            ),
            Err(EvalError::DivisionByZero)
        );
        assert_eq!(
            declared_numeric_arithmetic(
                Arithmetic::Multiply,
                &Value::Double(Double::from_float(f64::MAX)),
                &Value::Byte(2),
                declarations
            ),
            Err(EvalError::Overflow)
        );
        assert_eq!(
            declared_numeric_arithmetic(
                Arithmetic::Add,
                &Value::Currency(Currency::from_scaled(i64::MAX)),
                &Value::Byte(1),
                declarations
            ),
            Err(EvalError::Overflow)
        );
        assert_eq!(
            declared_numeric_arithmetic(
                Arithmetic::Add,
                &Value::Decimal(Decimal::new(2, 0).unwrap()),
                &Value::Byte(1),
                declarations
            ),
            Ok(Value::Decimal(Decimal::new(3, 0).unwrap()))
        );
        assert_eq!(
            declared_numeric_arithmetic(
                Arithmetic::Add,
                &Value::Int64(i64::MAX),
                &Value::Byte(1),
                declarations
            ),
            Err(EvalError::Overflow)
        );
    }
    assert_eq!(
        declared_numeric_arithmetic(
            Arithmetic::Add,
            &Value::Byte(255),
            &Value::Byte(1),
            context(false, false)
        ),
        Err(EvalError::Overflow)
    );
    assert_eq!(
        declared_numeric_arithmetic(
            Arithmetic::Add,
            &Value::Byte(255),
            &Value::Byte(1),
            context(true, false)
        ),
        Ok(Value::Int16(256))
    );
}

#[test]
fn date_envelope_overflow_depends_on_original_declarations() {
    let date = |serial| Value::DateTime(LegacyDateTime::from_ole_days(serial).unwrap());
    for (operator, left, right, result) in [
        (Arithmetic::Add, date(2958465.0), Value::Int16(1), 2958466.0),
        (Arithmetic::Add, Value::Int16(1), date(2958465.0), 2958466.0),
        (
            Arithmetic::Subtract,
            date(-657434.0),
            Value::Int16(1),
            -657435.0,
        ),
        (
            Arithmetic::Subtract,
            Value::Int16(0),
            date(2958465.0),
            -2958465.0,
        ),
        (Arithmetic::Add, date(2958465.0), date(1.0), 2958466.0),
    ] {
        assert!(uses_declared_overflow(operator, &left, &right));
        for (left_variant, right_variant) in
            [(false, false), (true, false), (false, true), (true, true)]
        {
            assert_eq!(
                declared_numeric_arithmetic(
                    operator,
                    &left,
                    &right,
                    context(left_variant, right_variant)
                ),
                if left_variant || right_variant {
                    Ok(Value::Double(Double::from_float(result)))
                } else {
                    Err(EvalError::Overflow)
                },
            );
        }
    }
    let left = date(2958465.0);
    let right = date(-657434.0);
    assert!(!uses_declared_overflow(Arithmetic::Subtract, &left, &right));
    assert_eq!(
        declared_numeric_arithmetic(Arithmetic::Subtract, &left, &right, context(false, false)),
        Ok(Value::Double(Double::from_float(3615899.0))),
    );
    assert_eq!(
        declared_numeric_arithmetic(
            Arithmetic::Add,
            &date(1.25),
            &Value::Int16(1),
            context(false, false)
        ),
        Ok(date(2.25)),
    );
    assert_eq!(
        declared_numeric_arithmetic(
            Arithmetic::Add,
            &date(1.0),
            &Value::Null,
            context(false, false)
        ),
        Ok(Value::Null),
    );
}

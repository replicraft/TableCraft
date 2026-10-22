use super::*;
use proptest::prelude::*;
use tablecraft_core::values::Double;

fn declarations(left: bool, right: bool) -> ArithmeticDeclarations {
    let declaration = |variant| {
        if variant {
            OperandDeclaration::Variant
        } else {
            OperandDeclaration::Scalar
        }
    };
    ArithmeticDeclarations {
        left: declaration(left),
        right: declaration(right),
    }
}

#[test]
fn declaration_categories_preserve_int_widths_and_control_overflow() {
    let cases = [
        (Value::Byte(255), Value::Byte(1), Value::Int16(256)),
        (Value::Int16(32767), Value::Int16(1), Value::Int32(32768)),
        (
            Value::Int32(2147483647),
            Value::Int32(1),
            Value::Double(Double::from_float(2147483648.0)),
        ),
    ];
    for (left, right, widened) in cases {
        for (a, b) in [(false, false), (false, true), (true, false), (true, true)] {
            let actual =
                declared_integral_arithmetic(Arithmetic::Add, &left, &right, declarations(a, b));
            assert_eq!(
                actual,
                if a || b {
                    Ok(widened.clone())
                } else {
                    Err(EvalError::Overflow)
                }
            );
        }
    }
    for context in [
        declarations(false, false),
        declarations(true, false),
        declarations(false, true),
        declarations(true, true),
    ] {
        assert_eq!(
            declared_integral_arithmetic(
                Arithmetic::Add,
                &Value::Int64(i64::MAX),
                &Value::Byte(1),
                context
            ),
            Err(EvalError::Overflow)
        );
        assert_eq!(
            declared_integral_arithmetic(
                Arithmetic::Divide,
                &Value::Int16(3),
                &Value::Int16(2),
                context
            ),
            Ok(Value::Double(Double::from_float(1.5)))
        );
        assert_eq!(
            declared_integral_arithmetic(Arithmetic::Add, &Value::Null, &Value::Byte(1), context),
            Ok(Value::Null)
        );
        assert_eq!(
            declared_integral_arithmetic(
                Arithmetic::Add,
                &Value::Null,
                &Value::Text("1".into()),
                context
            ),
            Err(EvalError::TypeMismatch)
        );
        assert_eq!(
            declared_integral_arithmetic(
                Arithmetic::IntegerDivide,
                &Value::Byte(1),
                &Value::Byte(0),
                context
            ),
            Err(EvalError::DivisionByZero)
        );
    }
    assert_eq!(
        declared_integral_arithmetic(
            Arithmetic::IntegerDivide,
            &Value::Int16(-32768),
            &Value::Int16(-1),
            declarations(false, false)
        ),
        Err(EvalError::Overflow)
    );
    assert_eq!(
        declared_integral_arithmetic(
            Arithmetic::IntegerDivide,
            &Value::Int16(-32768),
            &Value::Int16(-1),
            declarations(true, false)
        ),
        Ok(Value::Int32(32768))
    );
}

#[test]
fn every_byte_pair_matches_independent_wide_arithmetic_and_declared_bounds() {
    for a in 0..=255_u16 {
        for b in 0..=255_u16 {
            for (operator, result) in [
                (Arithmetic::Add, i32::from(a) + i32::from(b)),
                (Arithmetic::Subtract, i32::from(a) - i32::from(b)),
                (Arithmetic::Multiply, i32::from(a) * i32::from(b)),
            ] {
                let left = Value::Byte(a as u8);
                let right = Value::Byte(b as u8);
                let expected = if (0..=255).contains(&result) {
                    Ok(Value::Byte(result as u8))
                } else {
                    Err(EvalError::Overflow)
                };
                assert_eq!(
                    declared_integral_arithmetic(
                        operator,
                        &left,
                        &right,
                        declarations(false, false)
                    ),
                    expected
                );
                let widened = if (0..=255).contains(&result) {
                    Value::Byte(result as u8)
                } else if (-32768..=32767).contains(&result) {
                    Value::Int16(result as i16)
                } else {
                    Value::Int32(result)
                };
                assert_eq!(
                    declared_integral_arithmetic(
                        operator,
                        &left,
                        &right,
                        declarations(true, false)
                    ),
                    Ok(widened)
                );
            }
        }
    }
}

proptest! {
    #![proptest_config(crate::test_support::config())]
    #[test]
    fn typed_long_results_follow_independent_wide_integer_bounds(a in any::<i32>(), b in any::<i32>()) {
        for (operator, wide) in [
            (Arithmetic::Add, i128::from(a) + i128::from(b)),
            (Arithmetic::Subtract, i128::from(a) - i128::from(b)),
            (Arithmetic::Multiply, i128::from(a) * i128::from(b)),
        ] {
            let result = declared_integral_arithmetic(operator, &Value::Int32(a), &Value::Int32(b), declarations(false, false));
            if (-2147483648..=2147483647).contains(&wide) {
                prop_assert_eq!(result, Ok(Value::Int32(wide as i32)));
            } else {
                prop_assert_eq!(result, Err(EvalError::Overflow));
            }
        }
    }
}

use super::*;
use proptest::prelude::*;

fn double(value: f64) -> Value {
    Value::Double(Double::from_float(value))
}

#[test]
fn double_selected_operators_round_large_integral_inputs_before_arithmetic() {
    for (integer, rounded) in [
        (9007199254740993, 9007199254740992.0),
        (9007199254740995, 9007199254740996.0),
        (-9007199254740993, -9007199254740992.0),
        (i64::MAX, 9223372036854775808.0),
        (i64::MIN, -9223372036854775808.0),
    ] {
        let value = Value::Int64(integer);
        for floating in [double(1.0), Value::Single(Single::from_float(1.0))] {
            assert_eq!(
                promoted_arithmetic(Arithmetic::Add, &value, &floating),
                Ok(double(rounded + 1.0))
            );
            assert_eq!(
                promoted_arithmetic(Arithmetic::Subtract, &floating, &value),
                Ok(double(1.0 - rounded))
            );
            assert_eq!(
                promoted_arithmetic(Arithmetic::Multiply, &value, &floating),
                Ok(double(rounded))
            );
        }
        assert_eq!(
            promoted_arithmetic(Arithmetic::Divide, &value, &Value::Int64(integer)),
            Ok(double(1.0))
        );
        assert_eq!(
            promoted_arithmetic(Arithmetic::Power, &value, &Value::Byte(1)),
            Ok(double(rounded))
        );
        assert_eq!(
            promoted_arithmetic(Arithmetic::Power, &Value::Byte(1), &value),
            Ok(double(1.0))
        );
        assert_eq!(
            promoted_arithmetic(Arithmetic::Divide, &value, &Value::Byte(0)),
            Err(EvalError::DivisionByZero)
        );
        assert_eq!(
            arithmetic(Arithmetic::Divide, &value, &Value::Byte(1)),
            if integer as f64 as i128 == i128::from(integer) {
                Ok(double(rounded))
            } else {
                Err(EvalError::PrecisionLoss)
            }
        );
    }
    assert_eq!(
        promoted_arithmetic(Arithmetic::Add, &Value::Int64(i64::MAX), &Value::Byte(1)),
        Err(EvalError::Overflow)
    );
    assert_eq!(
        promoted_arithmetic(Arithmetic::Add, &Value::Int64(i64::MAX), &double(f64::NAN)),
        Err(EvalError::NonFinite)
    );
    assert_eq!(
        promoted_arithmetic(
            Arithmetic::Multiply,
            &Value::Int64(i64::MAX),
            &double(f64::MAX)
        ),
        Err(EvalError::Overflow)
    );
    assert_eq!(
        promoted_arithmetic(Arithmetic::Divide, &Value::Null, &Value::Byte(0)),
        Ok(Value::Null)
    );
}

// Independently round integer significands to 53 bits with ties to even.
fn rounded_integer(value: i64) -> f64 {
    let magnitude = value.unsigned_abs();
    let discarded = (u64::BITS - magnitude.leading_zeros()).saturating_sub(53);
    let unit = 1_u64 << discarded;
    let quotient = magnitude / unit;
    let remainder = magnitude % unit;
    let increment =
        remainder > unit / 2 || (discarded != 0 && remainder == unit / 2 && quotient % 2 == 1);
    let rounded = (quotient + u64::from(increment)) as f64 * unit as f64;
    if value < 0 { -rounded } else { rounded }
}

proptest! {
    #![proptest_config(crate::test_support::config())]
    #[test]
    fn integral_to_double_rounding_matches_independent_significand_model(value in any::<i64>()) {
        let expected = rounded_integer(value);
        prop_assert_eq!(
            promoted_arithmetic(Arithmetic::Multiply, &Value::Int64(value), &double(1.0)),
            Ok(double(expected))
        );
        prop_assert_eq!(
            promoted_arithmetic(Arithmetic::Divide, &Value::Int64(value), &Value::Byte(1)),
            Ok(double(expected))
        );
    }
}

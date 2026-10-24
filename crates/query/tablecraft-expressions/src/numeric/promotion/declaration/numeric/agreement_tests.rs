use super::*;
use tablecraft_core::values::{Double, LegacyDateTime, Single};

#[test]
fn every_byte_pair_matches_independent_integer_bounds_for_agreement() {
    for left in u8::MIN..=u8::MAX {
        for right in u8::MIN..=u8::MAX {
            let a = i32::from(left);
            let b = i32::from(right);
            for (operator, exact) in [
                (Arithmetic::Add, a + b),
                (Arithmetic::Subtract, a - b),
                (Arithmetic::Multiply, a * b),
            ] {
                let expected = u8::try_from(exact).ok().map(Value::Byte);
                assert_eq!(
                    arithmetic_agreement(operator, &Value::Byte(left), &Value::Byte(right)),
                    Ok(expected)
                );
            }
        }
    }
}

#[test]
fn rounded_quotients_shared_errors_and_other_selected_widths_remain_distinct() {
    let date = |serial| Value::DateTime(LegacyDateTime::from_ole_days(serial).unwrap());
    let single = |number| Value::Single(Single::from_float(number));
    for (operator, left, right, expected) in [
        (
            Arithmetic::Add,
            Value::Int16(2),
            Value::Int16(3),
            Some(Value::Int16(5)),
        ),
        (
            Arithmetic::Add,
            Value::Int16(i16::MAX),
            Value::Int16(1),
            None,
        ),
        (
            Arithmetic::Subtract,
            Value::Int32(i32::MIN),
            Value::Int16(1),
            None,
        ),
        (
            Arithmetic::IntegerDivide,
            Value::Int16(7),
            Value::Double(Double::from_float(2.5)),
            Some(Value::Int16(3)),
        ),
        (
            Arithmetic::IntegerDivide,
            Value::Int16(i16::MIN),
            Value::Int16(-1),
            None,
        ),
        (
            Arithmetic::Modulo,
            Value::Int16(i16::MIN),
            Value::Int16(-1),
            Some(Value::Int16(0)),
        ),
        (
            Arithmetic::Multiply,
            single(1.25),
            Value::Int16(2),
            Some(single(2.5)),
        ),
        (
            Arithmetic::Multiply,
            single(f32::MAX),
            Value::Int16(2),
            None,
        ),
        (
            Arithmetic::Add,
            date(2.25),
            Value::Int16(1),
            Some(date(3.25)),
        ),
        (Arithmetic::Add, date(2958465.0), Value::Int16(1), None),
        (
            Arithmetic::Subtract,
            date(2958465.0),
            date(-657434.0),
            Some(Value::Double(Double::from_float(3615899.0))),
        ),
        (
            Arithmetic::Add,
            Value::Null,
            Value::Int16(1),
            Some(Value::Null),
        ),
    ] {
        assert_eq!(arithmetic_agreement(operator, &left, &right), Ok(expected));
    }

    for (operator, left, right, expected) in [
        (
            Arithmetic::IntegerDivide,
            Value::Int16(1),
            Value::Int16(0),
            EvalError::DivisionByZero,
        ),
        (
            Arithmetic::Multiply,
            Value::Int64(i64::MAX),
            Value::Int16(2),
            EvalError::Overflow,
        ),
        (
            Arithmetic::Add,
            single(f32::INFINITY),
            Value::Null,
            EvalError::NonFinite,
        ),
        (
            Arithmetic::Add,
            Value::Text("2".into()),
            Value::Int16(1),
            EvalError::TypeMismatch,
        ),
    ] {
        assert_eq!(arithmetic_agreement(operator, &left, &right), Err(expected));
    }
}

use super::*;
use proptest::prelude::*;
use tablecraft_core::values::{Double, LegacyDateTime};

#[test]
fn widths_sign_extension_and_boolean_carriers_are_preserved() {
    let cases = [
        (Value::Byte(128), Value::Byte(1), Value::Byte(129)),
        (Value::Byte(128), Value::Boolean(true), Value::Int16(-1)),
        (Value::Int16(-32768), Value::Byte(1), Value::Int16(-32767)),
        (
            Value::Int32(i32::MIN),
            Value::Int16(1),
            Value::Int32(i32::MIN + 1),
        ),
        (
            Value::Int64(i64::MIN),
            Value::Int32(1),
            Value::Int64(i64::MIN + 1),
        ),
    ];
    for (a, b, expected) in cases {
        assert_eq!(Logical::Or.apply(&a, &b).unwrap(), expected);
        assert_eq!(Logical::Or.apply(&b, &a).unwrap(), expected);
    }
    assert_eq!(complement(&Value::Byte(0)), Ok(Value::Byte(255)));
    assert_eq!(complement(&Value::Int16(0)), Ok(Value::Int16(-1)));
    assert_eq!(complement(&Value::Int32(0)), Ok(Value::Int32(-1)));
    assert_eq!(complement(&Value::Int64(0)), Ok(Value::Int64(-1)));
    assert_eq!(complement(&Value::Boolean(false)), Ok(Value::Boolean(true)));
    assert_eq!(complement(&Value::Null), Ok(Value::Null));
    assert_eq!(
        Logical::And.apply(&Value::Int16(1), &Value::Int16(2)),
        Ok(Value::Int16(0))
    );
    assert_eq!(
        Logical::Implication.apply(&Value::Byte(1), &Value::Byte(2)),
        Ok(Value::Byte(254))
    );
    assert_eq!(
        Logical::Equivalence.apply(&Value::Byte(1), &Value::Byte(2)),
        Ok(Value::Byte(252))
    );
}

#[test]
fn boolean_null_tables_and_final_numeric_truth_stay_separate() {
    for (a, b, and, or) in [
        (
            Value::Boolean(false),
            Value::Null,
            Value::Boolean(false),
            Value::Null,
        ),
        (
            Value::Boolean(true),
            Value::Null,
            Value::Null,
            Value::Boolean(true),
        ),
        (Value::Null, Value::Null, Value::Null, Value::Null),
    ] {
        assert_eq!(Logical::And.apply(&a, &b).unwrap(), and);
        assert_eq!(Logical::Or.apply(&a, &b).unwrap(), or);
    }
    for (value, expected) in [
        (Value::Int64(i64::MIN), Truth::True),
        (Value::Byte(0), Truth::False),
        (Value::Double(Double::from_float(0.25)), Truth::True),
        (
            Value::DateTime(LegacyDateTime::from_ole_days(-0.25).unwrap()),
            Truth::True,
        ),
        (Value::Null, Truth::Unknown),
    ] {
        assert_eq!(Truth::from_predicate(&value), Ok(expected));
    }
    assert_eq!(
        Logical::And.apply(&Value::Int32(0), &Value::Null),
        Ok(Value::Int32(0))
    );
    assert_eq!(
        Logical::And.apply(&Value::Int16(1), &Value::Null),
        Ok(Value::Null)
    );
    assert_eq!(
        Logical::Or.apply(&Value::Null, &Value::Byte(3)),
        Ok(Value::Byte(3))
    );
    assert_eq!(
        Logical::Implication.apply(&Value::Null, &Value::Int64(3)),
        Ok(Value::Int64(3))
    );
    assert_eq!(
        Logical::Implication.apply(&Value::Int16(0), &Value::Null),
        Ok(Value::Int16(-1))
    );
    assert_eq!(
        Logical::Equivalence.apply(&Value::Int16(1), &Value::Null),
        Ok(Value::Null)
    );
    assert_eq!(
        Truth::from_value(&Value::Int16(1)),
        Err(EvalError::TypeMismatch)
    );
    assert_eq!(
        Truth::from_predicate(&Value::Text("1".into())),
        Err(EvalError::TypeMismatch)
    );
    for value in [
        Value::Text("1".into()),
        Value::Double(Double::from_float(1.0)),
    ] {
        assert_eq!(
            Logical::And.apply(&Value::Int16(0), &value),
            Err(EvalError::TypeMismatch)
        );
        assert_eq!(
            Logical::Or.apply(&value, &Value::Int16(1)),
            Err(EvalError::TypeMismatch)
        );
    }
}

proptest! {
    #[test]
    fn integer_bits_match_independent_per_bit_truth_tables(a: i64, b: i64) {
        for (operator, truth) in [
            (Logical::And, [false, false, false, true]),
            (Logical::Or, [false, true, true, true]),
            (Logical::Xor, [false, true, true, false]),
            (Logical::Equivalence, [true, false, false, true]),
            (Logical::Implication, [true, true, false, true]),
        ] {
            let mut expected = 0_u64;
            for position in 0..64 {
                let left = ((a as u64 >> position) & 1) as usize;
                let right = ((b as u64 >> position) & 1) as usize;
                if truth[left * 2 + right] {
                    expected |= 1_u64 << position;
                }
            }
            prop_assert_eq!(
                operator.apply(&Value::Int64(a), &Value::Int64(b)).unwrap(),
                Value::Int64(expected as i64),
            );
        }
        prop_assert_eq!(
            complement(&complement(&Value::Int64(a)).unwrap()).unwrap(),
            Value::Int64(a),
        );
    }
}

use super::{Logical, complement};
use crate::EvalError;
use proptest::prelude::*;
use tablecraft_core::values::{Currency, Decimal, Double, LegacyDateTime, Single, Value};

#[test]
fn half_even_float_fixed_and_date_inputs_share_long_operator_coercion() {
    for (coefficient, expected) in [
        (5, 0),
        (15, 2),
        (25, 2),
        (35, 4),
        (-5, 0),
        (-15, -2),
        (-25, -2),
    ] {
        for value in [
            Value::Single(Single::from_float(coefficient as f32 / 10.0)),
            Value::Double(Double::from_float(coefficient as f64 / 10.0)),
            Value::Currency(Currency::from_scaled(coefficient * 1000)),
            Value::Decimal(Decimal::new(i128::from(coefficient), 1).unwrap()),
            Value::DateTime(LegacyDateTime::from_ole_days(coefficient as f64 / 10.0).unwrap()),
        ] {
            assert_eq!(complement(&value), Ok(Value::Int32(!expected)));
            assert_eq!(
                Logical::Or.apply(&value, &Value::Byte(0)),
                Ok(Value::Int32(expected))
            );
            assert_eq!(
                Logical::Or.apply(&Value::Byte(0), &value),
                Ok(Value::Int32(expected))
            );
            assert_eq!(
                Logical::Or.apply(&value, &Value::Int64(0)),
                Ok(Value::Int64(i64::from(expected)))
            );
        }
    }
}

#[test]
fn result_width_overflow_and_null_do_not_hide_conversion_failures() {
    let huge = Value::Double(Double::from_float(2147483647.5));
    assert_eq!(complement(&huge), Err(EvalError::Overflow));
    assert_eq!(
        Logical::And.apply(&Value::Byte(0), &huge),
        Err(EvalError::Overflow)
    );
    assert_eq!(
        Logical::Xor.apply(&Value::Null, &huge),
        Err(EvalError::Overflow)
    );
    assert_eq!(
        Logical::Or.apply(&Value::Int64(0), &huge),
        Ok(Value::Int64(2147483648))
    );
    let exact = Value::Int64(9007199254740993);
    let zero = Value::Decimal(Decimal::new(0, 0).unwrap());
    assert_eq!(Logical::Or.apply(&exact, &zero), Ok(exact.clone()));
    assert_eq!(Logical::Or.apply(&zero, &exact), Ok(exact));
    let zero = Value::Double(Double::from_float(-0.0));
    assert_eq!(Logical::And.apply(&zero, &Value::Null), Ok(Value::Int32(0)));
    assert_eq!(Logical::And.apply(&Value::Null, &zero), Ok(Value::Int32(0)));
    for value in [f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
        let value = Value::Double(Double::from_float(value));
        assert_eq!(complement(&value), Err(EvalError::NonFinite));
        assert_eq!(
            Logical::Or.apply(&Value::Byte(1), &value),
            Err(EvalError::NonFinite)
        );
    }
    assert_eq!(
        complement(&Value::Text("1".into())),
        Err(EvalError::TypeMismatch)
    );
}

proptest! {
    #[test]
    fn exact_quarter_rounding_matches_nearest_even_candidate_search(q in -100_000_i32..100_000) {
        let lower = q.div_euclid(4);
        let expected = (lower..=lower + 1)
            .min_by_key(|candidate| ((q - candidate * 4).abs(), candidate.rem_euclid(2)))
            .unwrap();
        let serial = f64::from(q) / 4.0;
        for value in [
            Value::Double(Double::from_float(serial)),
            Value::Single(Single::from_float(serial as f32)),
            Value::Currency(Currency::from_scaled(i64::from(q) * 2500)),
            Value::Decimal(Decimal::new(i128::from(q) * 25, 2).unwrap()),
            Value::DateTime(LegacyDateTime::from_ole_days(serial).unwrap()),
        ] {
            prop_assert_eq!(complement(&value).unwrap(), Value::Int32(-expected - 1));
        }
    }
}

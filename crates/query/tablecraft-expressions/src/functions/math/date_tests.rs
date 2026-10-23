use super::MathFunction;
use crate::EvalError;
use proptest::prelude::*;
use tablecraft_core::values::{Double, ExtendedDateTime, LegacyDateTime, Value};

fn date(serial: f64) -> Value {
    Value::DateTime(LegacyDateTime::from_ole_days(serial).unwrap())
}

#[test]
fn date_floor_truncation_magnitude_and_sign_have_distinct_result_types() {
    for (serial, floor, trunc, magnitude, sign) in [
        (-1.75, -2.0, -1.0, 1.75, -1),
        (-0.25, -1.0, -0.0, 0.25, -1),
        (0.25, 0.0, 0.0, 0.25, 1),
        (2.75, 2.0, 2.0, 2.75, 1),
        (-0.0, -0.0, -0.0, 0.0, 0),
    ] {
        let input = date(serial);
        assert_eq!(
            MathFunction::Int.apply(std::slice::from_ref(&input)),
            Ok(date(floor))
        );
        assert_eq!(
            MathFunction::Fix.apply(std::slice::from_ref(&input)),
            Ok(date(trunc))
        );
        assert_eq!(
            MathFunction::Abs.apply(std::slice::from_ref(&input)),
            Ok(Value::Double(Double::from_float(magnitude)))
        );
        assert_eq!(MathFunction::Sgn.apply(&[input]), Ok(Value::Int16(sign)));
    }
}

#[test]
fn date_reconstruction_checks_the_legacy_envelope_and_extended_dates_stay_distinct() {
    assert_eq!(
        MathFunction::Int.apply(&[date(-657434.75)]),
        Err(EvalError::Overflow)
    );
    assert_eq!(
        MathFunction::Fix.apply(&[date(-657434.75)]),
        Ok(date(-657434.0))
    );
    assert_eq!(
        MathFunction::Int.apply(&[date(2958465.75)]),
        Ok(date(2958465.0))
    );
    assert_eq!(
        MathFunction::Abs.apply(&[date(-657434.75)]),
        Ok(Value::Double(Double::from_float(657434.75)))
    );
    for function in [
        MathFunction::Abs,
        MathFunction::Sgn,
        MathFunction::Int,
        MathFunction::Fix,
    ] {
        let extended =
            Value::DateTimeExtended(ExtendedDateTime::new(2026, 10, 23, 0, 0, 0, 0).unwrap());
        assert_eq!(function.apply(&[extended]), Err(EvalError::TypeMismatch));
    }
    assert_eq!(
        MathFunction::Round.apply(&[date(2.5)]),
        Err(EvalError::TypeMismatch)
    );
}

proptest! {
    #![proptest_config(crate::test_support::config())]
    #[test]
    fn quarter_day_serials_follow_independent_integer_floor_truncation_and_sign(quarters in -2_629_739_i64..11_833_864_i64) {
        let input = date(quarters as f64 / 4.0);
        let floor = quarters.div_euclid(4);
        let expected_floor = if floor < -657434 {
            Err(EvalError::Overflow)
        } else {
            Ok(date(floor as f64))
        };
        prop_assert_eq!(MathFunction::Int.apply(std::slice::from_ref(&input)), expected_floor);
        let truncated = quarters / 4;
        let signed_trunc = if truncated == 0 && quarters < 0 { -0.0 } else { truncated as f64 };
        prop_assert_eq!(MathFunction::Fix.apply(std::slice::from_ref(&input)), Ok(date(signed_trunc)));
        prop_assert_eq!(MathFunction::Sgn.apply(std::slice::from_ref(&input)), Ok(Value::Int16(quarters.signum() as i16)));
        prop_assert_eq!(MathFunction::Abs.apply(&[input]), Ok(Value::Double(Double::from_float(quarters.abs() as f64 / 4.0))));
    }
}

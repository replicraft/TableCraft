use super::*;
use proptest::prelude::*;
use tablecraft_core::values::{Currency, Decimal, Double, Single};

fn decimal(coefficient: i128, scale: u8) -> Value {
    Value::Decimal(Decimal::new(coefficient, scale).unwrap())
}
fn money(coefficient: i64) -> Value {
    Value::Currency(Currency::from_scaled(coefficient))
}
fn double(value: f64) -> Value {
    Value::Double(Double::from_float(value))
}
#[test]
fn unary_functions_retain_exact_carriers_and_report_abs_overflow() {
    for (negative, positive) in [
        (Value::Int16(-7), Value::Int16(7)),
        (Value::Int32(-7), Value::Int32(7)),
        (Value::Int64(-7), Value::Int64(7)),
        (money(-7), money(7)),
        (decimal(-7, 2), decimal(7, 2)),
        (
            Value::Single(Single::from_float(-7.0)),
            Value::Single(Single::from_float(7.0)),
        ),
        (double(-7.0), double(7.0)),
    ] {
        assert_eq!(MathFunction::Abs.apply(&[negative]), Ok(positive));
    }
    for value in [
        Value::Int16(i16::MIN),
        Value::Int32(i32::MIN),
        Value::Int64(i64::MIN),
        money(i64::MIN),
    ] {
        assert_eq!(MathFunction::Abs.apply(&[value]), Err(EvalError::Overflow));
    }
    assert_eq!(
        MathFunction::Abs.apply(&[Value::Byte(255)]),
        Ok(Value::Byte(255))
    );
    assert_eq!(
        MathFunction::Abs.apply(&[Value::Boolean(true)]),
        Ok(Value::Int16(1))
    );
    assert_eq!(
        MathFunction::Abs.apply(&[decimal(-Decimal::MAX_COEFFICIENT, 28)]),
        Ok(decimal(Decimal::MAX_COEFFICIENT, 28))
    );
}
#[test]
fn floor_and_truncation_differ_only_on_negative_fractional_values() {
    for (value, floor, trunc) in [
        (decimal(-84, 1), decimal(-9, 0), decimal(-8, 0)),
        (money(-84000), money(-90000), money(-80000)),
        (double(-8.4), double(-9.0), double(-8.0)),
    ] {
        assert_eq!(
            MathFunction::Int.apply(std::slice::from_ref(&value)),
            Ok(floor)
        );
        assert_eq!(MathFunction::Fix.apply(&[value]), Ok(trunc));
    }
    assert_eq!(
        MathFunction::Int.apply(&[money(i64::MIN)]),
        Err(EvalError::Overflow)
    );
    assert_eq!(
        MathFunction::Fix.apply(&[money(i64::MIN)]),
        Ok(money(-9223372036854770000))
    );
    assert_eq!(
        MathFunction::Int.apply(&[Value::Int64(i64::MIN)]),
        Ok(Value::Int64(i64::MIN))
    );
}
#[test]
fn signs_zero_null_and_invalid_carriers_do_not_use_lossy_conversion() {
    for (value, sign) in [
        (Value::Int64(i64::MIN), -1),
        (decimal(-1, 28), -1),
        (decimal(0, 28), 0),
        (Value::Int64(i64::MAX), 1),
        (double(-0.0), 0),
        (double(f64::from_bits(1)), 1),
        (Value::Boolean(true), -1),
    ] {
        assert_eq!(MathFunction::Sgn.apply(&[value]), Ok(Value::Int16(sign)));
    }
    for function in [
        MathFunction::Abs,
        MathFunction::Sgn,
        MathFunction::Int,
        MathFunction::Fix,
        MathFunction::Round,
    ] {
        assert_eq!(function.apply(&[Value::Null]), Ok(Value::Null));
        for value in [
            double(f64::NAN),
            double(f64::INFINITY),
            double(f64::NEG_INFINITY),
        ] {
            assert_eq!(function.apply(&[value]), Err(EvalError::NonFinite));
        }
        assert_eq!(
            function.apply(&[Value::Text("2.5".into())]),
            Err(EvalError::TypeMismatch)
        );
        assert_eq!(function.apply(&[]), Err(EvalError::UnsupportedExpression));
    }
}
#[test]
fn round_uses_half_even_and_checked_currency_reconstruction() {
    for (coefficient, expected) in [
        (125, 12),
        (135, 14),
        (-125, -12),
        (-135, -14),
        (5, 0),
        (-5, 0),
    ] {
        assert_eq!(
            MathFunction::Round.apply(&[decimal(coefficient, 2), Value::Int32(1)]),
            Ok(decimal(expected, 1))
        );
        assert_eq!(
            MathFunction::Round.apply(&[
                money(i64::try_from(coefficient * 100).unwrap()),
                Value::Int32(1)
            ]),
            Ok(money(i64::try_from(expected * 1000).unwrap()))
        );
    }
    assert_eq!(
        MathFunction::Round.apply(&[money(i64::MAX)]),
        Err(EvalError::Overflow)
    );
    assert_eq!(
        MathFunction::Round.apply(&[money(i64::MIN)]),
        Err(EvalError::Overflow)
    );
    assert_eq!(
        MathFunction::Round.apply(&[Value::Int64(i64::MAX), Value::Int32(28)]),
        Ok(Value::Int64(i64::MAX))
    );
    assert_eq!(
        MathFunction::Round.apply(&[decimal(12345678901234567890123456789, 0)]),
        Ok(decimal(12345678901234567890123456789, 0))
    );
}
#[test]
fn float_round_matches_documented_midpoints_and_preserves_large_or_already_precise_values() {
    for (value, expected) in [
        (0.12335, 0.1234),
        (0.12345, 0.1234),
        (0.12355, 0.1236),
        (0.12365, 0.1236),
        (0.00005, 0.0),
    ] {
        assert_eq!(
            MathFunction::Round.apply(&[double(value), Value::Int32(4)]),
            Ok(double(expected))
        );
    }
    for value in [0.1, -0.1, f64::MAX, -f64::MAX, 1e20, f64::MIN_POSITIVE] {
        let expected = if value.abs() < 1e-28 { 0.0 } else { value };
        assert_eq!(
            MathFunction::Round.apply(&[double(value), Value::Int32(28)]),
            Ok(double(expected))
        );
    }
    assert_eq!(MathFunction::Round.apply(&[double(-0.0)]), Ok(double(-0.0)));
    assert_eq!(
        MathFunction::Round.apply(&[Value::Single(Single::from_float(2.5))]),
        Ok(Value::Single(Single::from_float(2.0)))
    );
}
#[test]
fn place_count_validation_is_bounded_and_independent_of_null_value() {
    for count in [-1, 29, i32::MAX] {
        assert_eq!(
            MathFunction::Round.apply(&[Value::Null, Value::Int32(count)]),
            Err(EvalError::Domain)
        );
    }
    assert_eq!(
        MathFunction::Round.apply(&[Value::Null, Value::Null]),
        Err(EvalError::InvalidNullConversion)
    );
    assert_eq!(
        MathFunction::Round.apply(&[decimal(125, 2), double(1.5)]),
        Ok(decimal(125, 2))
    );
    for (name, function) in [
        ("aBs", MathFunction::Abs),
        ("SGN", MathFunction::Sgn),
        ("INT", MathFunction::Int),
        ("fiX", MathFunction::Fix),
        ("Round", MathFunction::Round),
    ] {
        assert_eq!(MathFunction::resolve(name), Some(function));
    }
}
proptest! {
    #![proptest_config(crate::test_support::config())]
    #[test]
    fn decimal_floor_truncate_and_round_match_integer_models(coefficient in any::<i64>(),scale in 0_u8..=18,places in 0_u8..=18){
        let input=decimal(i128::from(coefficient),scale);
        let factor=10_i128.pow(u32::from(scale));
        prop_assert_eq!(MathFunction::Int.apply(std::slice::from_ref(&input)),Ok(decimal(i128::from(coefficient).div_euclid(factor),0)));
        prop_assert_eq!(MathFunction::Fix.apply(std::slice::from_ref(&input)),Ok(decimal(i128::from(coefficient)/factor,0)));
        let result=MathFunction::Round.apply(&[input,Value::Byte(places)]).unwrap();
        if places>=scale{prop_assert_eq!(result,decimal(i128::from(coefficient),scale));}else{
            let divisor=10_i128.pow(u32::from(scale-places));
            let n=i128::from(coefficient);let q=n/divisor;let rem=(n%divisor).abs();
            let expected=q+if rem>divisor/2 || (rem==divisor/2 && q%2!=0){n.signum()}else{0};
            prop_assert_eq!(result,decimal(expected,places));
        }
    }
}

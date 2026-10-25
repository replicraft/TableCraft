use super::mixed_fixed_pair;
use num_bigint::BigInt;
use num_rational::BigRational;
use proptest::{prelude::*, test_runner::RngSeed};
use std::cmp::Ordering;
use tablecraft_core::values::{
    Currency, Decimal, Double, ExtendedDateTime, LegacyDateTime, Single, Value,
};
use tablecraft_expressions::EvalError;

fn decimal(coefficient: i128, scale: u8) -> Value {
    Value::Decimal(Decimal::new(coefficient, scale).unwrap())
}

fn double(bits: u64) -> Value {
    Value::Double(Double::from_bits(bits))
}

fn order(left: &Value, right: &Value) -> Result<Ordering, EvalError> {
    mixed_fixed_pair(left, right).unwrap().compare()
}

#[test]
fn exact_goldens_cover_rounding_boundaries_extremes_subnormals_and_zero_signs() {
    for (left, right, expected) in [
        (decimal(1, 1), double(0x3fb9_9999_9999_999a), Ordering::Less),
        (
            decimal(-1, 1),
            double(0xbfb9_9999_9999_999a),
            Ordering::Greater,
        ),
        (decimal(1, 28), double(1), Ordering::Greater),
        (
            decimal(-1, 28),
            double(0x8000_0000_0000_0001),
            Ordering::Less,
        ),
        (
            decimal(79_228_162_514_264_337_593_543_950_335, 0),
            double(0x45f0_0000_0000_0000),
            Ordering::Less,
        ),
        (
            decimal(1, 28),
            double(0x7fef_ffff_ffff_ffff),
            Ordering::Less,
        ),
        (
            Value::Int64(9_007_199_254_740_993),
            double(0x4340_0000_0000_0000),
            Ordering::Greater,
        ),
        (
            Value::Int64(i64::MAX),
            double(0x43e0_0000_0000_0000),
            Ordering::Less,
        ),
        (
            Value::Int64(i64::MIN),
            double(0xc3e0_0000_0000_0000),
            Ordering::Equal,
        ),
        (
            Value::Currency(Currency::from_scaled(5000)),
            double(0x3fe0_0000_0000_0000),
            Ordering::Equal,
        ),
        (
            Value::Boolean(true),
            double(0xbff0_0000_0000_0000),
            Ordering::Equal,
        ),
        (
            Value::Byte(255),
            double(0xbff0_0000_0000_0000),
            Ordering::Greater,
        ),
        (
            decimal(0, 28),
            double(0x8000_0000_0000_0000),
            Ordering::Equal,
        ),
        (Value::Int16(0), double(0), Ordering::Equal),
        (
            Value::Currency(Currency::from_scaled(0)),
            double(1),
            Ordering::Less,
        ),
    ] {
        assert_eq!(order(&left, &right), Ok(expected), "{left:?}, {right:?}");
        assert_eq!(order(&right, &left), Ok(expected.reverse()));
    }
}

#[test]
fn nonfinite_pairs_refuse_and_unrelated_families_do_not_acquire_numeric_admission() {
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let binary = Value::Double(Double::from_float(value));
        assert_eq!(order(&decimal(0, 0), &binary), Err(EvalError::NonFinite));
        assert_eq!(order(&binary, &Value::Int64(0)), Err(EvalError::NonFinite));
    }
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let binary = Value::Single(Single::from_float(value));
        assert_eq!(order(&decimal(0, 0), &binary), Err(EvalError::NonFinite));
    }
    for outside in [
        Value::Null,
        Value::Text("1".into()),
        Value::Binary(vec![1]),
        Value::DateTime(LegacyDateTime::from_ole_days(1.0).unwrap()),
        Value::DateTimeExtended(ExtendedDateTime::new(2000, 1, 1, 0, 0, 0, 0).unwrap()),
    ] {
        assert!(mixed_fixed_pair(&outside, &double(0)).is_none());
        assert!(mixed_fixed_pair(&double(0), &outside).is_none());
    }
    assert!(mixed_fixed_pair(&decimal(1, 0), &Value::Int16(1)).is_none());
    assert!(mixed_fixed_pair(&double(0), &double(1)).is_none());
}

fn rational(value: &Value) -> BigRational {
    let (coefficient, scale) = match value {
        Value::Boolean(value) => (-BigInt::from(u8::from(*value)), 0),
        Value::Byte(value) => (BigInt::from(*value), 0),
        Value::Int16(value) => (BigInt::from(*value), 0),
        Value::Int32(value) => (BigInt::from(*value), 0),
        Value::Int64(value) => (BigInt::from(*value), 0),
        Value::Currency(value) => (BigInt::from(value.scaled()), 4),
        Value::Decimal(value) => (BigInt::from(value.coefficient()), u32::from(value.scale())),
        Value::Single(value) => return BigRational::from_float(value.to_float()).unwrap(),
        Value::Double(value) => return BigRational::from_float(value.to_float()).unwrap(),
        _ => panic!("non-numeric oracle input"),
    };
    BigRational::new(coefficient, BigInt::from(10).pow(scale))
}

fn fixed_values() -> impl Strategy<Value = Value> {
    prop_oneof![
        any::<bool>().prop_map(Value::Boolean),
        any::<u8>().prop_map(Value::Byte),
        any::<i16>().prop_map(Value::Int16),
        any::<i32>().prop_map(Value::Int32),
        any::<i64>().prop_map(Value::Int64),
        any::<i64>().prop_map(|value| Value::Currency(Currency::from_scaled(value))),
        (
            0_i128..=79_228_162_514_264_337_593_543_950_335,
            any::<bool>(),
            0_u8..=28
        )
            .prop_map(|(coefficient, negative, scale)| {
                if negative {
                    return decimal(-coefficient, scale);
                }
                decimal(coefficient, scale)
            }),
    ]
}

fn binary_values() -> impl Strategy<Value = Value> {
    prop_oneof![
        any::<u32>()
            .prop_filter("finite Single", |bits| bits & 0x7f80_0000 != 0x7f80_0000)
            .prop_map(|bits| Value::Single(Single::from_bits(bits))),
        any::<u64>()
            .prop_filter("finite Double", |bits| bits & 0x7ff0_0000_0000_0000
                != 0x7ff0_0000_0000_0000)
            .prop_map(double),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 512,
        rng_seed: RngSeed::Fixed(0x6b65_795f_6669_7865),
        ..ProptestConfig::default()
    })]

    #[test]
    fn full_fixed_and_binary_ranges_match_independent_arbitrary_rationals(
        fixed in fixed_values(),
        binary in binary_values(),
    ) {
        let expected = rational(&fixed).cmp(&rational(&binary));
        prop_assert_eq!(order(&fixed, &binary), Ok(expected));
        prop_assert_eq!(order(&binary, &fixed), Ok(expected.reverse()));
    }

    #[test]
    fn fixed_peers_preserve_transitivity_through_a_binary_middle(
        first in fixed_values(),
        middle in binary_values(),
        last in fixed_values(),
    ) {
        let before = order(&first, &middle).unwrap();
        let after = order(&middle, &last).unwrap();
        let expected = rational(&first).cmp(&rational(&last));
        if before != Ordering::Greater && after != Ordering::Greater {
            prop_assert_ne!(expected, Ordering::Greater);
        }
        if before != Ordering::Less && after != Ordering::Less {
            prop_assert_ne!(expected, Ordering::Less);
        }
        if before == Ordering::Equal && after == Ordering::Equal {
            prop_assert_eq!(expected, Ordering::Equal);
        }
    }

    #[test]
    fn exact_dyadic_decimal_values_distinguish_adjacent_double_bits(
        numerator in 1_u32..=16_777_215,
        scale in 0_u8..=28,
        negative in any::<bool>(),
    ) {
        let coefficient = i128::from(numerator) * 5_i128.pow(u32::from(scale));
        let coefficient = if negative {
            -coefficient
        } else {
            coefficient
        };
        let fixed = decimal(coefficient, scale);

        let signed = if negative {
            -f64::from(numerator)
        } else {
            f64::from(numerator)
        };
        let exact = signed / 2_f64.powi(i32::from(scale));
        let bits = exact.to_bits();
        prop_assert_eq!(order(&fixed, &double(bits)), Ok(Ordering::Equal));

        let earlier = if negative {
            bits + 1
        } else {
            bits - 1
        };
        let later = if negative {
            bits - 1
        } else {
            bits + 1
        };
        prop_assert_eq!(order(&fixed, &double(earlier)), Ok(Ordering::Greater));
        prop_assert_eq!(order(&fixed, &double(later)), Ok(Ordering::Less));
    }
}

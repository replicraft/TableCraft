use super::mixed_float_views;
use proptest::prelude::*;
use std::cmp::Ordering;
use tablecraft_core::values::{
    Currency, Decimal, Double, ExtendedDateTime, Guid, LegacyDateTime, Single, Value,
};
use tablecraft_expressions::{
    EvalError,
    comparison::{Comparison, compare},
    logic::Truth,
};

const SINGLE_FRACTION_BITS: u32 = 23;
const DOUBLE_FRACTION_BITS: u32 = 52;
const SINGLE_EXPONENT_BIAS: i32 = 127;
const DOUBLE_EXPONENT_BIAS: i32 = 1023;
const SINGLE_SUBNORMAL_EXPONENT: i32 = 1 - SINGLE_EXPONENT_BIAS - SINGLE_FRACTION_BITS as i32;
const SINGLE_EXPONENT_MASK: u32 = 0x7f80_0000;
const DOUBLE_EXPONENT_MASK: u64 = 0x7ff0_0000_0000_0000;
const DOUBLE_SIGN_MASK: u64 = 1 << (u64::BITS - 1);

fn single(bits: u32) -> Value {
    Value::Single(Single::from_bits(bits))
}

fn double(bits: u64) -> Value {
    Value::Double(Double::from_bits(bits))
}

// Derive binary64 fields from binary32 fields without a floating-point cast.
fn widened_bits(bits: u32) -> u64 {
    let sign = u64::from(bits >> (u32::BITS - 1)) << (u64::BITS - 1);
    let exponent = (bits & SINGLE_EXPONENT_MASK) >> SINGLE_FRACTION_BITS;
    let fraction = bits & ((1 << SINGLE_FRACTION_BITS) - 1);

    if exponent == 0 {
        if fraction == 0 {
            return sign;
        }

        let leading = u32::BITS - 1 - fraction.leading_zeros();
        let exponent =
            u64::try_from(leading as i32 + SINGLE_SUBNORMAL_EXPONENT + DOUBLE_EXPONENT_BIAS)
                .unwrap();
        let fraction = u64::from(fraction ^ (1 << leading)) << (DOUBLE_FRACTION_BITS - leading);
        return sign | (exponent << DOUBLE_FRACTION_BITS) | fraction;
    }

    assert!(exponent < (SINGLE_EXPONENT_MASK >> SINGLE_FRACTION_BITS));
    let exponent =
        u64::try_from(exponent as i32 - SINGLE_EXPONENT_BIAS + DOUBLE_EXPONENT_BIAS).unwrap();
    sign | (exponent << DOUBLE_FRACTION_BITS)
        | (u64::from(fraction) << (DOUBLE_FRACTION_BITS - SINGLE_FRACTION_BITS))
}

fn exact_bits(value: &Value) -> u64 {
    match value {
        Value::Single(value) => widened_bits(value.to_bits()),
        Value::Double(value) => value.to_bits(),
        Value::Int32(value) => {
            if *value == 0 {
                return 0;
            }

            let magnitude = i64::from(*value).unsigned_abs();
            let leading = u64::BITS - 1 - magnitude.leading_zeros();
            let fraction = (magnitude ^ (1 << leading)) << (DOUBLE_FRACTION_BITS - leading);
            let sign = if *value < 0 { DOUBLE_SIGN_MASK } else { 0 };
            let exponent = u64::try_from(leading as i32 + DOUBLE_EXPONENT_BIAS).unwrap();
            sign | (exponent << DOUBLE_FRACTION_BITS) | fraction
        }
        _ => panic!("binary floating or Int32 input required"),
    }
}

// Finite IEEE bit fields admit an integer order after sign normalization.
// Both zero representations occupy the same numeric equivalence class.
fn numeric_rank(bits: u64) -> u64 {
    let bits = if bits & !DOUBLE_SIGN_MASK == 0 {
        0
    } else {
        bits
    };

    if bits & DOUBLE_SIGN_MASK == 0 {
        bits | DOUBLE_SIGN_MASK
    } else {
        !bits
    }
}

fn expected_order(left: &Value, right: &Value) -> Ordering {
    numeric_rank(exact_bits(left)).cmp(&numeric_rank(exact_bits(right)))
}

fn key_comparison(operation: Comparison, left: &Value, right: &Value) -> Result<Truth, EvalError> {
    if let Some((left, right)) = mixed_float_views(left, right) {
        compare(operation, &left, &right, None)
    } else {
        compare(operation, left, right, None)
    }
}

fn key_order(left: &Value, right: &Value) -> Ordering {
    if key_comparison(Comparison::Equal, left, right).unwrap() == Truth::True {
        Ordering::Equal
    } else if key_comparison(Comparison::Less, left, right).unwrap() == Truth::True {
        Ordering::Less
    } else {
        Ordering::Greater
    }
}

fn finite_single_bits() -> impl Strategy<Value = u32> {
    any::<u32>().prop_filter("finite binary32", |bits| {
        bits & SINGLE_EXPONENT_MASK != SINGLE_EXPONENT_MASK
    })
}

fn finite_double_bits() -> impl Strategy<Value = u64> {
    any::<u64>().prop_filter("finite binary64", |bits| {
        bits & DOUBLE_EXPONENT_MASK != DOUBLE_EXPONENT_MASK
    })
}

fn finite_binary_value() -> impl Strategy<Value = Value> {
    prop_oneof![
        finite_single_bits().prop_map(single),
        finite_double_bits().prop_map(double),
    ]
}

#[test]
fn golden_widening_preserves_zero_sign_subnormals_normal_limits_and_double_bits() {
    for (source, expected) in [
        (0x0000_0000, 0x0000_0000_0000_0000),
        (0x8000_0000, 0x8000_0000_0000_0000),
        (0x0000_0001, 0x36a0_0000_0000_0000),
        (0x007f_ffff, 0x380f_ffff_c000_0000),
        (0x0080_0000, 0x3810_0000_0000_0000),
        (0x3f80_0000, 0x3ff0_0000_0000_0000),
        (0x7f7f_ffff, 0x47ef_ffff_e000_0000),
        (0xff7f_ffff, 0xc7ef_ffff_e000_0000),
    ] {
        let peer = double(0x3ff0_0000_02af_31dc);
        let (left, right) = mixed_float_views(&single(source), &peer).unwrap();
        assert_eq!(left, double(expected));
        assert_eq!(right, peer);

        let (left, right) = mixed_float_views(&peer, &single(source)).unwrap();
        assert_eq!(left, peer);
        assert_eq!(right, double(expected));
    }

    for (value, bits) in [
        (0, 0x0000_0000_0000_0000),
        (1, 0x3ff0_0000_0000_0000),
        (-1, 0xbff0_0000_0000_0000),
        (i32::MIN, 0xc1e0_0000_0000_0000),
        (i32::MAX, 0x41df_ffff_ffc0_0000),
    ] {
        assert_eq!(exact_bits(&Value::Int32(value)), bits);
    }
}

#[test]
fn all_other_carrier_pairs_keep_their_existing_admission_path() {
    let values = [
        Value::Null,
        Value::Boolean(true),
        Value::Byte(255),
        Value::Int16(-1),
        Value::Int32(1),
        Value::Int64(i64::MAX),
        Value::Currency(Currency::from_scaled(1)),
        Value::Decimal(Decimal::new(1, 28).unwrap()),
        Value::DateTime(LegacyDateTime::from_ole_days(-0.25).unwrap()),
        Value::DateTimeExtended(ExtendedDateTime::new(2026, 1, 1, 0, 0, 0, 1).unwrap()),
        Value::Text("1".into()),
        Value::Binary(vec![1]),
        Value::Guid(Guid::from_bytes([1; 16])),
    ];

    for left in &values {
        for right in &values {
            assert!(mixed_float_views(left, right).is_none());
        }
        for binary in [single(0x3f80_0000), double(0x3ff0_0000_0000_0000)] {
            assert!(mixed_float_views(left, &binary).is_none());
            assert!(mixed_float_views(&binary, left).is_none());
        }
    }

    assert!(mixed_float_views(&single(0), &single(1)).is_none());
    assert!(mixed_float_views(&double(0), &double(1)).is_none());
}

#[test]
fn views_do_not_hide_nonfinite_errors_or_change_scalar_predicate_narrowing() {
    for nonfinite in [0x7f80_0000, 0xff80_0000, 0x7fc0_0001, 0xff80_0001] {
        for (left, right) in [
            (single(nonfinite), double(0)),
            (double(0), single(nonfinite)),
        ] {
            assert!(mixed_float_views(&left, &right).is_some());
            assert_eq!(
                key_comparison(Comparison::Equal, &left, &right),
                Err(EvalError::NonFinite)
            );
        }
    }

    for nonfinite in [
        0x7ff0_0000_0000_0000,
        0xfff0_0000_0000_0000,
        0x7ff8_0000_0000_0001,
        0xfff0_0000_0000_0001,
    ] {
        for (left, right) in [
            (single(0), double(nonfinite)),
            (double(nonfinite), single(0)),
        ] {
            assert_eq!(
                key_comparison(Comparison::Equal, &left, &right),
                Err(EvalError::NonFinite)
            );
        }
    }

    for value in [single(0x7f80_0000), single(0x7fc0_0001)] {
        assert!(mixed_float_views(&value, &single(0)).is_none());
        assert_eq!(
            key_comparison(Comparison::Equal, &value, &single(0)),
            Err(EvalError::NonFinite)
        );
    }
    for value in [double(0x7ff0_0000_0000_0000), double(0x7ff8_0000_0000_0001)] {
        assert!(mixed_float_views(&value, &double(0)).is_none());
        assert_eq!(
            key_comparison(Comparison::Equal, &value, &double(0)),
            Err(EvalError::NonFinite)
        );
    }

    let left = single(0x3f80_0000);
    let right = double(0x3ff0_0000_0000_0001);
    assert_eq!(
        compare(Comparison::Equal, &left, &right, None),
        Ok(Truth::True)
    );
    assert_eq!(
        key_comparison(Comparison::Equal, &left, &right),
        Ok(Truth::False)
    );
    assert_eq!(key_order(&left, &right), Ordering::Less);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn finite_widening_matches_independent_ieee_fields(
        left in finite_single_bits(),
        right in finite_double_bits(),
    ) {
        let source = single(left);
        let peer = double(right);
        let (widened, retained) = mixed_float_views(&source, &peer).unwrap();
        prop_assert_eq!(widened, double(widened_bits(left)));
        prop_assert_eq!(retained, peer.clone());

        let (retained, widened) = mixed_float_views(&peer, &source).unwrap();
        prop_assert_eq!(widened, double(widened_bits(left)));
        prop_assert_eq!(retained, peer);
    }

    #[test]
    fn finite_binary_order_matches_exact_integer_ranks(
        left in finite_binary_value(),
        right in finite_binary_value(),
    ) {
        let expected = expected_order(&left, &right);
        let actual = key_order(&left, &right);
        prop_assert_eq!(actual, expected);
        prop_assert_eq!(key_order(&right, &left), actual.reverse());
        prop_assert_eq!(key_order(&left, &left), Ordering::Equal);

        prop_assert_eq!(
            key_comparison(Comparison::Equal, &left, &right).unwrap() == Truth::True,
            actual == Ordering::Equal,
        );
        prop_assert_eq!(
            key_comparison(Comparison::LessOrEqual, &left, &right).unwrap() == Truth::True,
            actual != Ordering::Greater,
        );
        prop_assert_eq!(
            key_comparison(Comparison::Greater, &left, &right).unwrap() == Truth::True,
            actual == Ordering::Greater,
        );
    }

    #[test]
    fn finite_binary_triples_are_transitive(
        first in finite_binary_value(),
        second in finite_binary_value(),
        third in finite_binary_value(),
    ) {
        let ab = key_order(&first, &second);
        let bc = key_order(&second, &third);
        let ac = key_order(&first, &third);

        if ab != Ordering::Greater && bc != Ordering::Greater {
            prop_assert_ne!(ac, Ordering::Greater);
        }
        if ab == Ordering::Equal && bc == Ordering::Equal {
            prop_assert_eq!(ac, Ordering::Equal);
        }
    }

    #[test]
    fn nearby_double_classes_do_not_merge_through_a_single_bridge(
        source in finite_single_bits(),
    ) {
        let widened = widened_bits(source);
        let adjacent = if widened & !DOUBLE_SIGN_MASK == 0 {
            1
        } else if widened & DOUBLE_SIGN_MASK == 0 {
            widened + 1
        } else {
            widened - 1
        };
        let first = single(source);
        let second = double(widened);
        let third = double(adjacent);

        prop_assert_eq!(key_order(&first, &second), Ordering::Equal);
        prop_assert_eq!(key_order(&second, &third), Ordering::Less);
        prop_assert_eq!(key_order(&first, &third), Ordering::Less);
    }

    #[test]
    fn mixed_binary_pairs_keep_exact_integral_third_value_order(
        first in finite_single_bits(),
        second in finite_double_bits(),
        third in any::<i32>(),
    ) {
        let values = [single(first), double(second), Value::Int32(third)];
        for left in &values {
            for right in &values {
                prop_assert_eq!(key_order(left, right), expected_order(left, right));
            }
        }

        for [a, b, c] in [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ] {
            let ab = key_order(&values[a], &values[b]);
            let bc = key_order(&values[b], &values[c]);
            if ab != Ordering::Greater && bc != Ordering::Greater {
                prop_assert_ne!(key_order(&values[a], &values[c]), Ordering::Greater);
            }
        }
    }
}

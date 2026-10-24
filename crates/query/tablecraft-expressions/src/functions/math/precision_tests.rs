use super::MathFunction;
use crate::EvalError;
use proptest::prelude::*;
use tablecraft_core::values::{Currency, Decimal, Double, Single, Value};

fn round(value: Value, places: i32) -> Result<Value, EvalError> {
    MathFunction::Round.apply(&[value, Value::Int32(places)])
}

fn double(value: f64) -> Value {
    Value::Double(Double::from_float(value))
}

#[test]
fn high_precision_counts_keep_fixed_carriers_scales_and_float_bits() {
    for places in [29, 308, 309, 323, 324, i32::MAX] {
        for value in [
            Value::Byte(255),
            Value::Int16(-32768),
            Value::Int32(i32::MAX),
            Value::Int64(i64::MIN),
            Value::Currency(Currency::from_scaled(i64::MAX)),
            Value::Currency(Currency::from_scaled(i64::MIN)),
            Value::Decimal(Decimal::new(12300, 4).unwrap()),
            Value::Decimal(Decimal::new(1, 28).unwrap()),
            double(-0.0),
            double(0.125),
            double(f64::MAX),
            Value::Single(Single::from_bits(0x8000_0000)),
        ] {
            assert_eq!(round(value.clone(), places), Ok(value), "{places}");
        }
    }
}

#[test]
fn tiny_ieee_values_round_without_overflowing_the_decimal_factor() {
    for (value, places, expected) in [
        (1e-30, 29, 0.0),
        (1e-30, 30, 1e-30),
        (2_f64.powi(-100), 30, 1e-30),
        (2_f64.powi(-100), 31, 8e-31),
        (f64::from_bits(1), 323, 0.0),
        (f64::from_bits(2), 323, f64::from_bits(2)),
        (f64::from_bits(3), 323, f64::from_bits(2)),
        (f64::from_bits(5), 323, f64::from_bits(4)),
        (f64::from_bits(1), 324, f64::from_bits(1)),
    ] {
        for sign in [1.0, -1.0] {
            assert_eq!(
                round(double(sign * value), places),
                Ok(double(sign * expected)),
                "{value:e}/{places}/{sign}"
            );
        }
    }
    let tiny_single = Value::Single(Single::from_bits(1));
    assert_eq!(round(tiny_single.clone(), 45), Ok(tiny_single));
    assert_eq!(
        round(Value::Single(Single::from_bits(1)), 44),
        Ok(Value::Single(Single::from_bits(0)))
    );
}

#[test]
fn long_count_conversion_precedes_source_admission_without_a_scale_ceiling() {
    assert_eq!(round(Value::Null, i32::MAX), Ok(Value::Null));
    assert_eq!(
        MathFunction::Round.apply(&[Value::Null, Value::Int64(i64::from(i32::MAX) + 1),]),
        Err(EvalError::Overflow)
    );
    assert_eq!(round(Value::Null, -1), Err(EvalError::Domain));
    assert_eq!(
        round(Value::Text("bad".into()), 29),
        Err(EvalError::TypeMismatch)
    );
    assert_eq!(round(double(f64::NAN), 324), Err(EvalError::NonFinite));
}

proptest! {
    #![proptest_config(crate::test_support::config())]

    #[test]
    fn decimal_places_finer_than_half_a_subnormal_preserve_every_finite_bit_pattern(
        bits in any::<u64>(),
        places in 324_i32..=i32::MAX,
    ) {
        let value = f64::from_bits(bits);
        prop_assume!(value.is_finite());
        prop_assert_eq!(round(double(value), places), Ok(double(value)));
    }
}

use super::*;
use std::cmp::Ordering;

pub(crate) fn compare(left: &Value, right: &Value) -> Result<Ordering, EvalError> {
    if let (Some(left), Some(right)) = (fixed(left), fixed(right)) {
        return Ok(fixed_compare(left, right));
    }
    let mut a = float(left)?;
    let mut b = float(right)?;
    // Microsoft's comparison rule: Double is rounded to Single precision.
    if matches!(
        (left, right),
        (Value::Single(_), Value::Double(_)) | (Value::Double(_), Value::Single(_))
    ) {
        a = f64::from(a as f32);
        b = f64::from(b as f32);
        if !a.is_finite() || !b.is_finite() {
            return Err(EvalError::Overflow);
        }
    }
    a.partial_cmp(&b).ok_or(EvalError::NonFinite)
}

#[cfg(test)]
mod properties {
    use super::*;
    use proptest::prelude::*;
    use tablecraft_core::values::Decimal;
    proptest! {
        #![proptest_config(crate::test_support::config())]
        #[test]
        fn scale_and_integer_carriers_do_not_change_numeric_order(a in any::<i64>(), b in any::<i64>(), scale in 0_u8..=9) {
            let scaled = Value::Decimal(Decimal::new(i128::from(a) * 10_i128.pow(u32::from(scale)), scale).unwrap());
            prop_assert_eq!(compare(&scaled, &Value::Int64(a)), Ok(Ordering::Equal));
            prop_assert_eq!(compare(&scaled, &Value::Int64(b)), Ok(a.cmp(&b)));
        }
    }
}

// Compare whole parts and decimal digits without overflowing scale alignment.
fn fixed_compare((a, sa): (i128, u8), (b, sb): (i128, u8)) -> Ordering {
    let sign = a.signum().cmp(&b.signum());
    if sign != Ordering::Equal {
        return sign;
    }
    let negative = a < 0;
    let (a, b) = (a.unsigned_abs(), b.unsigned_abs());
    let (da, db) = (10_u128.pow(u32::from(sa)), 10_u128.pow(u32::from(sb)));
    let mut order = (a / da).cmp(&(b / db));
    let (mut ra, mut rb) = (a % da, b % db);
    for _ in 0..sa.max(sb) {
        if order != Ordering::Equal {
            break;
        }
        ra *= 10;
        rb *= 10;
        order = (ra / da).cmp(&(rb / db));
        ra %= da;
        rb %= db;
    }
    if negative { order.reverse() } else { order }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tablecraft_core::values::{Currency, Decimal, Double, Single};
    #[test]
    fn exact_comparison_ignores_carrier_and_scale_without_losing_large_digits() {
        let decimal = |c, s| Value::Decimal(Decimal::new(c, s).unwrap());
        for (a, b, expected) in [
            (decimal(10, 1), decimal(100, 2), Ordering::Equal),
            (
                Value::Int64(9_007_199_254_740_993),
                decimal(9_007_199_254_740_992, 0),
                Ordering::Greater,
            ),
            (
                decimal(Decimal::MAX_COEFFICIENT, 0),
                decimal(1, 28),
                Ordering::Greater,
            ),
            (decimal(-1, 28), Value::Byte(0), Ordering::Less),
            (decimal(-123, 2), decimal(-1229, 3), Ordering::Less),
            (
                Value::Currency(Currency::from_scaled(12_000)),
                decimal(120, 2),
                Ordering::Equal,
            ),
            (Value::Boolean(true), Value::Int16(-1), Ordering::Equal),
        ] {
            assert_eq!(compare(&a, &b), Ok(expected));
            assert_eq!(compare(&b, &a), Ok(expected.reverse()));
        }
    }
    #[test]
    fn floats_compare_numerically_and_mixed_precision_is_explicit() {
        let double = |n| Value::Double(Double::from_float(n));
        assert_eq!(compare(&double(0.0), &double(-0.0)), Ok(Ordering::Equal));
        assert_eq!(
            compare(
                &Value::Single(Single::from_float(1.0)),
                &double(1.0 + f64::EPSILON)
            ),
            Ok(Ordering::Equal)
        );
        assert_eq!(
            compare(&double(f64::NAN), &double(1.0)),
            Err(EvalError::NonFinite)
        );
        assert_eq!(
            compare(&Value::Int64(9_007_199_254_740_993), &double(1.0)),
            Err(EvalError::PrecisionLoss)
        );
        assert_eq!(
            compare(&Value::Currency(Currency::from_scaled(1)), &double(1.0)),
            Err(EvalError::UnsupportedNumericCombination)
        );
    }
}

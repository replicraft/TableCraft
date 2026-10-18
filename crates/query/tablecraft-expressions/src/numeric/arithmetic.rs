use super::*;
use tablecraft_core::values::{Currency, Decimal, Double};

/// Numeric operations supported by the initial scalar profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arithmetic {
    /// +, numeric operands only.
    Add,
    /// -.
    Subtract,
    /// *.
    Multiply,
    /// /, finite Double result for the supported integer/float domain.
    Divide,
    /// \, half-even integral operands and truncated quotient.
    IntegerDivide,
    /// Mod, half-even integral operands and signed remainder.
    Modulo,
    /// ^, finite Double result.
    Power,
}

#[cfg(test)]
mod properties {
    use super::*;
    use proptest::prelude::*;
    proptest! {
        #![proptest_config(crate::test_support::config())]
        #[test]
        fn integer_results_match_wide_mathematical_arithmetic(a in any::<i64>(), b in any::<i64>()) {
            for (operator, expected) in [
                (Arithmetic::Add, i128::from(a) + i128::from(b)),
                (Arithmetic::Subtract, i128::from(a) - i128::from(b)),
                (Arithmetic::Multiply, i128::from(a) * i128::from(b)),
            ] {
                let expected = i64::try_from(expected).map(Value::Int64).map_err(|_| EvalError::Overflow);
                prop_assert_eq!(arithmetic(operator, &Value::Int64(a), &Value::Int64(b)), expected);
            }
        }
        #[test]
        fn rounded_division_retains_the_signed_remainder_identity(a in any::<i64>(), b in any::<i64>()) {
            prop_assume!(b != 0 && !(a == i64::MIN && b == -1));
            let Value::Int64(q) = arithmetic(Arithmetic::IntegerDivide, &Value::Int64(a), &Value::Int64(b)).unwrap() else { unreachable!() };
            let Value::Int64(r) = arithmetic(Arithmetic::Modulo, &Value::Int64(a), &Value::Int64(b)).unwrap() else { unreachable!() };
            prop_assert_eq!(i128::from(q) * i128::from(b) + i128::from(r), i128::from(a));
            prop_assert!(r.unsigned_abs() < b.unsigned_abs());
            prop_assert!(r == 0 || r.signum() == a.signum());
        }
    }
}
/// Evaluate checked numeric operands. Integral results use Int64, floating
/// results use Double. This API does not infer VBA Variant promotion types.
/// Decimal arithmetic and Currency/float mixing are explicit unsupported cases.
pub fn arithmetic(operator: Arithmetic, left: &Value, right: &Value) -> Result<Value, EvalError> {
    if matches!(left, Value::Null) || matches!(right, Value::Null) {
        return Ok(Value::Null);
    }
    if matches!(operator, Arithmetic::IntegerDivide | Arithmetic::Modulo) {
        let (a, b) = (coercion::rounded(left)?, coercion::rounded(right)?);
        let (a, b) = (
            i64::try_from(a).map_err(|_| EvalError::Overflow)?,
            i64::try_from(b).map_err(|_| EvalError::Overflow)?,
        );
        if b == 0 {
            return Err(EvalError::DivisionByZero);
        }
        let result = if operator == Arithmetic::IntegerDivide {
            i128::from(a) / i128::from(b)
        } else {
            i128::from(a) % i128::from(b)
        };
        return checked_integer(result);
    }
    if matches!(left, Value::Decimal(_)) || matches!(right, Value::Decimal(_)) {
        return Err(EvalError::UnsupportedNumericCombination);
    }
    if matches!(left, Value::Currency(_)) || matches!(right, Value::Currency(_)) {
        return currency(operator, left, right);
    }
    if let (Some(a), Some(b)) = (integer(left), integer(right)) {
        let (a, b) = (i128::from(a), i128::from(b));
        match operator {
            Arithmetic::Add => return checked_integer(a + b),
            Arithmetic::Subtract => return checked_integer(a - b),
            Arithmetic::Multiply => return checked_integer(a * b),
            _ => {}
        }
    }
    let (a, b) = (float(left)?, float(right)?);
    let result = match operator {
        Arithmetic::Add => a + b,
        Arithmetic::Subtract => a - b,
        Arithmetic::Multiply => a * b,
        Arithmetic::Divide => {
            if b == 0.0 {
                return Err(EvalError::DivisionByZero);
            }
            a / b
        }
        Arithmetic::Power => {
            if a == 0.0 && b < 0.0 {
                return Err(EvalError::DivisionByZero);
            }
            a.powf(b)
        }
        Arithmetic::IntegerDivide | Arithmetic::Modulo => unreachable!("handled above"),
    };
    finite(result)
}
/// Checked numeric negation; NULL stays NULL. Integer results use Int64.
pub fn negate(value: &Value) -> Result<Value, EvalError> {
    match value {
        Value::Null => Ok(Value::Null),
        Value::Currency(value) => value
            .scaled()
            .checked_neg()
            .map(|n| Value::Currency(Currency::from_scaled(n)))
            .ok_or(EvalError::Overflow),
        Value::Decimal(value) => Ok(Value::Decimal(
            Decimal::new(-value.coefficient(), value.scale()).map_err(|_| EvalError::Overflow)?,
        )),
        _ => {
            if let Some(number) = integer(value) {
                checked_integer(-i128::from(number))
            } else {
                finite(-float(value)?)
            }
        }
    }
}
fn checked_integer(number: i128) -> Result<Value, EvalError> {
    i64::try_from(number)
        .map(Value::Int64)
        .map_err(|_| EvalError::Overflow)
}
fn finite(number: f64) -> Result<Value, EvalError> {
    if number.is_nan() {
        Err(EvalError::Domain)
    } else if number.is_finite() {
        Ok(Value::Double(Double::from_float(number)))
    } else {
        Err(EvalError::Overflow)
    }
}
fn currency(operator: Arithmetic, left: &Value, right: &Value) -> Result<Value, EvalError> {
    let scaled = |value: &Value| match value {
        Value::Currency(value) => Ok(i128::from(value.scaled())),
        _ => integer(value)
            .map(|n| i128::from(n) * 10_000)
            .ok_or(EvalError::UnsupportedNumericCombination),
    };
    let result = match operator {
        Arithmetic::Add => scaled(left)? + scaled(right)?,
        Arithmetic::Subtract => scaled(left)? - scaled(right)?,
        Arithmetic::Multiply => match (left, right) {
            (Value::Currency(a), b) | (b, Value::Currency(a)) if integer(b).is_some() => {
                i128::from(a.scaled()) * i128::from(integer(b).unwrap())
            }
            _ => return Err(EvalError::UnsupportedNumericCombination),
        },
        _ => return Err(EvalError::UnsupportedNumericCombination),
    };
    i64::try_from(result)
        .map(|n| Value::Currency(Currency::from_scaled(n)))
        .map_err(|_| EvalError::Overflow)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn arithmetic_is_checked_and_null_is_not_zero() {
        use Arithmetic::*;
        let int = Value::Int64;
        assert_eq!(
            arithmetic(Add, &int(i64::MAX), &int(1)),
            Err(EvalError::Overflow)
        );
        assert_eq!(
            arithmetic(Multiply, &int(i64::MIN), &int(-1)),
            Err(EvalError::Overflow)
        );
        assert_eq!(
            arithmetic(IntegerDivide, &int(i64::MIN), &int(-1)),
            Err(EvalError::Overflow)
        );
        assert_eq!(arithmetic(Modulo, &int(i64::MIN), &int(-1)), Ok(int(0)));
        assert_eq!(arithmetic(IntegerDivide, &int(-11), &int(4)), Ok(int(-2)));
        assert_eq!(arithmetic(Modulo, &int(-11), &int(4)), Ok(int(-3)));
        assert_eq!(
            arithmetic(Divide, &int(1), &int(0)),
            Err(EvalError::DivisionByZero)
        );
        assert_eq!(arithmetic(Divide, &Value::Null, &int(0)), Ok(Value::Null));
        assert_eq!(negate(&int(i64::MIN)), Err(EvalError::Overflow));
        assert_eq!(arithmetic(Add, &Value::Boolean(true), &int(2)), Ok(int(1)));
        let double = |n| Value::Double(Double::from_float(n));
        assert_eq!(arithmetic(Modulo, &double(12.6), &double(5.0)), Ok(int(3)));
        assert_eq!(
            arithmetic(IntegerDivide, &double(9.5), &double(2.5)),
            Ok(int(5))
        );
        assert_eq!(
            arithmetic(Multiply, &double(f64::MAX), &double(2.0)),
            Err(EvalError::Overflow)
        );
        assert_eq!(
            arithmetic(Add, &double(f64::NAN), &int(1)),
            Err(EvalError::NonFinite)
        );
    }
    #[test]
    fn currency_stays_exact_and_unimplemented_rounding_is_explicit() {
        let money = |n| Value::Currency(Currency::from_scaled(n));
        assert_eq!(
            arithmetic(Arithmetic::Add, &money(1), &money(2)),
            Ok(money(3))
        );
        assert_eq!(
            arithmetic(Arithmetic::Subtract, &money(10_000), &Value::Int32(1)),
            Ok(money(0))
        );
        assert_eq!(
            arithmetic(Arithmetic::Multiply, &money(1), &Value::Int64(3)),
            Ok(money(3))
        );
        assert_eq!(
            arithmetic(Arithmetic::Add, &money(i64::MAX), &money(1)),
            Err(EvalError::Overflow)
        );
        assert_eq!(
            arithmetic(Arithmetic::Multiply, &money(1), &money(1)),
            Err(EvalError::UnsupportedNumericCombination)
        );
    }
}

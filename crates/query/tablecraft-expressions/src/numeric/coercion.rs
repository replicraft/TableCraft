use super::*;

/// Explicit integral target. These conversions round halves to even.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntegerType {
    /// Access Byte.
    Byte,
    /// Access Integer.
    Int16,
    /// Access Long.
    Int32,
    /// Large Number carrier; native backend availability is checked elsewhere.
    Int64,
}
/// Convert numeric/Boolean carriers to a chosen integer width; NULL stays NULL.
/// Text, dates and language-only states have no implicit conversion here.
pub fn coerce_integer(value: &Value, target: IntegerType) -> Result<Value, EvalError> {
    if matches!(value, Value::Null) {
        return Ok(Value::Null);
    }
    let number = rounded(value)?;
    match target {
        IntegerType::Byte => u8::try_from(number)
            .map(Value::Byte)
            .map_err(|_| EvalError::Overflow),
        IntegerType::Int16 => i16::try_from(number)
            .map(Value::Int16)
            .map_err(|_| EvalError::Overflow),
        IntegerType::Int32 => i32::try_from(number)
            .map(Value::Int32)
            .map_err(|_| EvalError::Overflow),
        IntegerType::Int64 => i64::try_from(number)
            .map(Value::Int64)
            .map_err(|_| EvalError::Overflow),
    }
}
pub(super) fn rounded(value: &Value) -> Result<i128, EvalError> {
    if let Some((coefficient, scale)) = fixed(value) {
        let divisor = 10_i128.pow(u32::from(scale));
        let whole = coefficient / divisor;
        let twice = (coefficient % divisor).abs() * 2;
        return Ok(whole
            + if twice > divisor || (twice == divisor && whole % 2 != 0) {
                coefficient.signum()
            } else {
                0
            });
    }
    let value = float(value)?.round_ties_even();
    // Bounds are exclusive at +2^63: i64::MAX rounds to that value as f64.
    if !(-9_223_372_036_854_775_808.0..9_223_372_036_854_775_808.0).contains(&value) {
        return Err(EvalError::Overflow);
    }
    Ok(value as i128)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tablecraft_core::values::{Decimal, Double};
    #[test]
    fn half_even_rounding_and_width_boundaries_are_explicit() {
        for (n, expected) in [(5, 0), (15, 2), (25, 2), (-5, 0), (-15, -2), (-25, -2)] {
            for value in [
                Value::Decimal(Decimal::new(n, 1).unwrap()),
                Value::Double(Double::from_float(n as f64 / 10.0)),
            ] {
                assert_eq!(
                    coerce_integer(&value, IntegerType::Int16),
                    Ok(Value::Int16(expected))
                );
            }
        }
        assert_eq!(
            coerce_integer(&Value::Boolean(true), IntegerType::Int32),
            Ok(Value::Int32(-1))
        );
        assert_eq!(
            coerce_integer(&Value::Boolean(true), IntegerType::Byte),
            Err(EvalError::Overflow)
        );
        assert_eq!(
            coerce_integer(&Value::Int64(i64::MAX), IntegerType::Int64),
            Ok(Value::Int64(i64::MAX))
        );
        assert_eq!(
            coerce_integer(
                &Value::Double(Double::from_float(i64::MAX as f64)),
                IntegerType::Int64
            ),
            Err(EvalError::Overflow)
        );
        assert_eq!(
            coerce_integer(&Value::Text("2".into()), IntegerType::Int16),
            Err(EvalError::TypeMismatch)
        );
        assert_eq!(
            coerce_integer(&Value::Null, IntegerType::Int16),
            Ok(Value::Null)
        );
    }
}

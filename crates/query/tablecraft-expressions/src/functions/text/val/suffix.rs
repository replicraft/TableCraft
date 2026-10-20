//! Type-suffix validation, kept separate from ordinary implicit coercion.
use crate::functions::conversions::text::scaled::rounded;
use crate::{EvalError, functions::conversions::Conversion};
use tablecraft_core::values::{Currency, Double, Value};

pub(super) fn decimal(text: &str, suffix: Option<char>) -> Result<Double, EvalError> {
    match suffix {
        Some('%' | '&' | '^') => {
            let value = rounded(text, 0, false)?;
            let valid = match suffix {
                Some('%') => i16::try_from(value).is_ok(),
                Some('&') => i32::try_from(value).is_ok(),
                _ => i64::try_from(value).is_ok(),
            };
            if !valid {
                return Err(EvalError::Overflow);
            }
            Ok(Double::from_float(value as f64))
        }
        Some('@') => {
            let coefficient = i64::try_from(rounded(text, i64::from(Currency::SCALE), true)?)
                .map_err(|_| EvalError::Overflow)?;
            let Value::Double(value) =
                Conversion::Double.apply(&Value::Currency(Currency::from_scaled(coefficient)))?
            else {
                return Err(EvalError::TypeMismatch);
            };
            Ok(value)
        }
        Some('$') => Err(EvalError::TypeMismatch),
        _ => finish(
            text.parse::<f64>().map_err(|_| EvalError::TypeMismatch)?,
            suffix,
        ),
    }
}
pub(super) fn radix(value: i128, suffix: Option<char>) -> Result<Double, EvalError> {
    if suffix == Some('$') {
        return Err(EvalError::TypeMismatch);
    }
    if suffix == Some('@') {
        // The signed radix value is integral; checked Currency conversion supplies
        // the existing four-place/range policy, without a second arithmetic engine.
        let value = i64::try_from(value).map_err(|_| EvalError::Overflow)?;
        let currency = Conversion::Currency.apply(&Value::Int64(value))?;
        let Value::Double(value) = Conversion::Double.apply(&currency)? else {
            return Err(EvalError::TypeMismatch);
        };
        return Ok(value);
    }
    finish(value as f64, suffix)
}
fn finish(value: f64, suffix: Option<char>) -> Result<Double, EvalError> {
    if !value.is_finite() {
        return Err(EvalError::Overflow);
    }
    if suffix == Some('!') {
        let single = value as f32;
        if !single.is_finite() {
            return Err(EvalError::Overflow);
        }
        return Ok(Double::from_float(f64::from(single)));
    }
    Ok(Double::from_float(value))
}

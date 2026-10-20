//! Explicit regional numeric Text conversion with whole-input validation.
mod decimal;
mod normalize;
mod policy;
pub(crate) mod scaled;
use super::Conversion;
use crate::{EvalError, functions::text::output::NumericBuffer};
pub use policy::NumberFormat;
use tablecraft_core::values::{Currency, Double, Single, Value};

pub(super) fn convert(
    conversion: Conversion,
    text: &str,
    format: NumberFormat,
    buffer: &mut impl NumericBuffer,
) -> Result<Value, EvalError> {
    if !buffer.text().is_empty() {
        return Err(EvalError::Domain);
    }
    // Charge complete validation scans and check cancellation before normalization.
    for _ in text.chars() {
        buffer.step()?;
    }
    if conversion == Conversion::Boolean {
        let text = text.trim();
        if text.eq_ignore_ascii_case("true") || text == "#TRUE#" {
            return Ok(Value::Boolean(true));
        }
        if text.eq_ignore_ascii_case("false") || text == "#FALSE#" {
            return Ok(Value::Boolean(false));
        }
    }
    normalize::write(text, format, buffer)?;
    for _ in 0..buffer.text().len() {
        buffer.step()?;
    }
    let text = buffer.text();
    match conversion {
        Conversion::Single => {
            let number = text.parse::<f32>().map_err(|_| EvalError::TypeMismatch)?;
            if !number.is_finite() {
                return Err(EvalError::Overflow);
            }
            Ok(Value::Single(Single::from_float(number)))
        }
        Conversion::Double | Conversion::Boolean => {
            let number = text.parse::<f64>().map_err(|_| EvalError::TypeMismatch)?;
            if !number.is_finite() {
                return Err(EvalError::Overflow);
            }
            if conversion == Conversion::Boolean {
                Ok(Value::Boolean(number != 0.0))
            } else {
                Ok(Value::Double(Double::from_float(number)))
            }
        }
        Conversion::Currency => {
            let coefficient =
                i64::try_from(scaled::rounded(text, i64::from(Currency::SCALE), true)?)
                    .map_err(|_| EvalError::Overflow)?;
            Ok(Value::Currency(Currency::from_scaled(coefficient)))
        }
        Conversion::Decimal => decimal::convert(text),
        _ => {
            let integer =
                i64::try_from(scaled::rounded(text, 0, true)?).map_err(|_| EvalError::Overflow)?;
            conversion.apply(&Value::Int64(integer))
        }
    }
}
#[cfg(test)]
mod tests;

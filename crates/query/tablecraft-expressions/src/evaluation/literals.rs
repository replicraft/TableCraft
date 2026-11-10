//! Numeric spelling evaluation; carrier ranges govern representability.
mod radix;
use crate::EvalError;
use tablecraft_core::values::{Double, Value};

pub(super) fn number(text: &str) -> Result<Value, EvalError> {
    if text.starts_with('&') {
        return radix::parse(text);
    }
    if !text.contains(['.', 'e', 'E'])
        || !text
            .bytes()
            .all(|b| b.is_ascii_digit() || b"+-.eE".contains(&b))
    {
        return Err(EvalError::UnsupportedLiteral);
    }
    let value: f64 = text.parse().map_err(|_| EvalError::UnsupportedLiteral)?;
    if !value.is_finite() {
        return Err(EvalError::Overflow);
    }
    let mantissa = text
        .split(['e', 'E'])
        .next()
        .ok_or(EvalError::UnsupportedLiteral)?;
    if value == 0.0 && mantissa.bytes().any(|b| (b'1'..=b'9').contains(&b)) {
        return Err(EvalError::PrecisionLoss);
    }
    Ok(Value::Double(Double::from_float(value)))
}

#[cfg(test)]
mod tests;

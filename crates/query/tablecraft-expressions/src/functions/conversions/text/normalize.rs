//! Whole-string numeric grammar, distinct from Val's numeric-prefix scanner.
use super::NumberFormat;
use crate::{EvalError, functions::text::output::NumericBuffer};

const GROUP_DIGITS: usize = 3;
const MAX_PREFIX_TOKENS: usize = 2;

pub(super) fn write(
    text: &str,
    format: NumberFormat,
    output: &mut impl NumericBuffer,
) -> Result<(), EvalError> {
    let mut body = text.trim();
    let parentheses = body.starts_with('(') && body.ends_with(')');
    if parentheses {
        body = body[1..body.len() - 1].trim();
    }
    let mut negative = parentheses;
    let mut signed = parentheses;
    let mut currency = false;
    // A sign may precede or follow the single currency token.
    for _ in 0..MAX_PREFIX_TOKENS {
        if body.starts_with(['+', '-']) {
            if signed {
                return Err(EvalError::TypeMismatch);
            }
            negative = body.starts_with('-');
            signed = true;
            body = body[1..].trim_start();
        } else if let Some(token) = format.currency().filter(|token| body.starts_with(*token)) {
            if currency {
                return Err(EvalError::TypeMismatch);
            }
            currency = true;
            body = body[token.len_utf8()..].trim_start();
        } else {
            break;
        }
    }
    if let Some(token) = format.currency().filter(|token| body.ends_with(*token)) {
        if currency {
            return Err(EvalError::TypeMismatch);
        }
        body = body[..body.len() - token.len_utf8()].trim_end();
    }
    if negative {
        output.append("-")?;
    }
    let (mantissa, exponent) = match body.find(['e', 'E', 'd', 'D']) {
        Some(index) => (&body[..index], Some(&body[index + 1..])),
        None => (body, None),
    };
    let (whole, fraction) = mantissa
        .split_once(format.decimal())
        .map_or((mantissa, None), |(a, b)| (a, Some(b)));
    let whole = whole.trim();
    let mut digits = 0;
    if let Some(group) = format.group().filter(|group| whole.contains(*group)) {
        for (index, part) in whole.split(group).enumerate() {
            let part = part.trim();
            if (index == 0 && !(1..=GROUP_DIGITS).contains(&part.len()))
                || (index > 0 && part.len() != GROUP_DIGITS)
            {
                return Err(EvalError::TypeMismatch);
            }
            digits += append_digits(part, output)?;
        }
    } else if !whole.is_empty() {
        digits += append_digits(whole, output)?;
    }
    if let Some(fraction) = fraction {
        output.append(".")?;
        let fraction = fraction.trim();
        if !fraction.is_empty() {
            digits += append_digits(fraction, output)?;
        }
    }
    if digits == 0 {
        return Err(EvalError::TypeMismatch);
    }
    if let Some(exponent) = exponent {
        output.append("e")?;
        let mut exponent = exponent.trim();
        if exponent.starts_with(['+', '-']) {
            output.append(&exponent[..1])?;
            exponent = exponent[1..].trim_start();
        }
        append_digits(exponent, output)?;
    }
    Ok(())
}
fn append_digits(text: &str, output: &mut impl NumericBuffer) -> Result<usize, EvalError> {
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(EvalError::TypeMismatch);
    }
    output.append(text)?;
    Ok(text.len())
}

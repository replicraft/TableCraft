//! Solve the real positive-base annuity equation for its period count.
use crate::EvalError;

pub(super) fn calculate(
    rate: f64,
    payment: f64,
    present: f64,
    future: f64,
    timing: f64,
) -> Result<f64, EvalError> {
    let scale = payment.abs().max(present.abs()).max(future.abs());
    if scale == 0.0 {
        return Err(EvalError::Domain);
    }
    let payment = payment / scale;
    let present = present / scale;
    let future = future / scale;
    if rate == 0.0 {
        return if payment == 0.0 {
            Err(EvalError::DivisionByZero)
        } else {
            Ok(-(present + future) / payment)
        };
    }
    if rate <= -1.0 {
        return Err(EvalError::Domain);
    }
    let due = 1.0 + rate * timing;
    let denominator = present * rate + payment * due;
    if !denominator.is_finite() {
        return Err(EvalError::Overflow);
    }
    if denominator == 0.0 {
        return Err(EvalError::Domain);
    }
    let total = present + future;
    let relative = -total * (rate / denominator);
    let logarithm = rate.ln_1p();
    if relative.is_finite() && relative > -1.0 {
        let correction = if relative == 0.0 {
            1.0
        } else {
            relative.ln_1p() / relative
        };
        return Ok(correction * (-total / denominator) * (rate / logarithm));
    }
    // A ratio can overflow or round to zero before its logarithm is taken.
    let numerator = payment * due - future * rate;
    if !numerator.is_finite() {
        return Err(EvalError::Overflow);
    }
    if numerator == 0.0 || numerator.is_sign_negative() != denominator.is_sign_negative() {
        return Err(EvalError::Domain);
    }
    Ok((numerator.abs().ln() - denominator.abs().ln()) / logarithm)
}

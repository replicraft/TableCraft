//! Cashflow equation: pv*g + pmt*(1+rate*timing)*(g-1)/rate + fv = 0.
use crate::EvalError;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Annuity {
    Payment,
    Present,
    Future,
}

pub(super) fn calculate(
    function: Annuity,
    rate: f64,
    periods: f64,
    cash: f64,
    balance: f64,
    timing: f64,
) -> Result<f64, EvalError> {
    if rate == 0.0 {
        return match function {
            Annuity::Payment => payment(cash, balance, 1.0, periods),
            Annuity::Present | Annuity::Future => Ok(-sum(cash * periods, balance)),
        };
    }
    let due = 1.0 + rate * timing;
    if rate > -1.0 {
        let exponent = periods * rate.ln_1p();
        return match function {
            // Discount the equation for positive exponents. Neither PV nor Pmt
            // needs to overflow solely because the undiscounted growth does.
            Annuity::Present => Ok(-sum(
                product(balance, (-exponent).exp()),
                product(cash, due * annuity(rate, periods, exponent, true)),
            )),
            Annuity::Future => Ok(-sum(
                product(balance, exponent.exp()),
                product(cash, due * annuity(rate, periods, exponent, false)),
            )),
            Annuity::Payment if exponent >= 0.0 => payment(
                cash,
                balance,
                (-exponent).exp(),
                due * annuity(rate, periods, exponent, true),
            ),
            Annuity::Payment => payment(
                balance,
                cash,
                exponent.exp(),
                due * annuity(rate, periods, exponent, false),
            ),
        };
    }
    // A negative growth base has a real power only for integral periods.
    if rate < -1.0 && periods.fract() != 0.0 {
        return Err(EvalError::Domain);
    }
    let growth = (1.0 + rate).powf(periods);
    let annuity = due * ((growth - 1.0) / rate);
    match function {
        Annuity::Payment => divide(-sum(product(cash, growth), balance), annuity),
        Annuity::Present => divide(-sum(product(cash, annuity), balance), growth),
        Annuity::Future => Ok(-sum(product(cash, annuity), product(balance, growth))),
    }
}
fn divide(numerator: f64, denominator: f64) -> Result<f64, EvalError> {
    if denominator == 0.0 {
        Err(EvalError::DivisionByZero)
    } else {
        Ok(numerator / denominator)
    }
}
fn product(cash: f64, factor: f64) -> f64 {
    if cash == 0.0 { 0.0 } else { cash * factor }
}
fn sum(a: f64, b: f64) -> f64 {
    a + b
}

// Factoring through exponent avoids dividing two separately rounded subnormal
// quantities when the rate or period count is near zero.
fn annuity(rate: f64, periods: f64, exponent: f64, discounted: bool) -> f64 {
    let signed = if discounted { -exponent } else { exponent };
    if exponent.abs() <= f64::EPSILON.sqrt() {
        let relative = if signed == 0.0 {
            1.0
        } else {
            signed.exp_m1() / signed
        };
        periods * (rate.ln_1p() / rate) * relative
    } else if discounted {
        -signed.exp_m1() / rate
    } else {
        signed.exp_m1() / rate
    }
}

// Normalize balances before summing/dividing: their sum may exceed Double even
// when a payment spread across several periods remains representable.
fn payment(a: f64, b: f64, factor: f64, divisor: f64) -> Result<f64, EvalError> {
    if divisor == 0.0 {
        return Err(EvalError::DivisionByZero);
    }
    let scale = a.abs().max(b.abs());
    if scale == 0.0 {
        return Ok(0.0);
    }
    Ok(-((a / scale + product(b / scale, factor)) / divisor) * scale)
}

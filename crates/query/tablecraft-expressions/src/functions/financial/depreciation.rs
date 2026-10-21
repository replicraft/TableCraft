//! Original algebraic kernels; no schedule allocation or iteration.
use super::FinancialFunction;
use crate::EvalError;

const SHORT_LIFE_BOUNDARY: f64 = 2.0;

pub(super) fn calculate(
    function: FinancialFunction,
    cost: f64,
    salvage: f64,
    life: f64,
    period: f64,
    factor: f64,
) -> Result<f64, EvalError> {
    match function {
        FinancialFunction::SLN => straight(cost, salvage, life),
        FinancialFunction::SYD => {
            if salvage < 0.0 || period <= 0.0 || period > life {
                return Err(EvalError::Domain);
            }
            let baseline = straight(cost, salvage, life)?;
            // Divide before multiplying. life*(life+1) may overflow although the
            // weighted charge remains finite; subtract period before adding one.
            Ok((baseline * (((life - period) + 1.0) / (life + 1.0))) * 2.0)
        }
        FinancialFunction::DDB => declining(cost, salvage, life, period, factor),
        _ => Err(EvalError::UnsupportedExpression),
    }
}
fn straight(cost: f64, salvage: f64, life: f64) -> Result<f64, EvalError> {
    if life == 0.0 {
        return Err(EvalError::DivisionByZero);
    }
    let difference = cost - salvage;
    if difference.is_finite() {
        return Ok(difference / life);
    }
    let scale = cost.abs().max(salvage.abs());
    Ok(((cost / scale - salvage / scale) / life) * scale)
}
fn declining(
    cost: f64,
    salvage: f64,
    life: f64,
    period: f64,
    factor: f64,
) -> Result<f64, EvalError> {
    if salvage < 0.0 || factor <= 0.0 || period <= 0.0 || period > life {
        return Err(EvalError::Domain);
    }
    if cost <= 0.0 {
        return Ok(0.0);
    }
    // Provisional legacy-runtime boundaries, separately documented from ordinary
    // declining-balance schedule agreement and native Access qualification.
    if life < SHORT_LIFE_BOUNDARY {
        return Ok(cost - salvage);
    }
    if life == SHORT_LIFE_BOUNDARY {
        return Ok(if period <= 1.0 { cost - salvage } else { 0.0 });
    }
    let rate = factor / life;
    let remaining = cost - salvage;
    if period <= 1.0 {
        let nominal = if rate == 0.0 {
            (cost / life) * factor
        } else {
            cost * rate
        };
        return Ok(nominal.min(remaining));
    }
    let elapsed = period - 1.0;
    let growth = if rate < 1.0 {
        (elapsed * (-rate).ln_1p()).exp()
    } else {
        let base = (life - factor) / life;
        if base < 0.0 && elapsed.fract() != 0.0 {
            return Err(EvalError::Domain);
        }
        base.powf(elapsed)
    };
    let book = cost * growth;
    if !book.is_finite() {
        return Err(EvalError::Overflow);
    }
    let nominal = if rate == 0.0 {
        (book / life) * factor
    } else {
        book * rate
    };
    Ok(nominal.min(book - salvage).max(0.0))
}

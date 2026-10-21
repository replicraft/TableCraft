//! Constant-work payment components; never walk an unbounded amortization schedule.
use super::{
    FinancialFunction,
    annuity::{self, Annuity},
};
use crate::EvalError;

pub(super) fn calculate(
    function: FinancialFunction,
    rate: f64,
    per: f64,
    periods: f64,
    present: f64,
    future: f64,
    timing: f64,
) -> Result<f64, EvalError> {
    if per < 1.0 || per > periods {
        return Err(EvalError::Domain);
    }
    let payment = annuity::calculate(Annuity::Payment, rate, periods, present, future, timing)?;
    if !payment.is_finite() {
        return Err(EvalError::Overflow);
    }
    let interest = if rate == 0.0 || (timing == 1.0 && per == 1.0) {
        0.0
    } else if rate > 0.0 && !(timing == 0.0 && per == 1.0) {
        // Discount remaining payments instead of subtracting large accumulated
        // balances late in a positive-rate loan. Beginning payments accrue their
        // preceding interest one period before the terminal balance is due.
        let terminal = if timing == 1.0 {
            future / (1.0 + rate)
        } else {
            future
        };
        -annuity::calculate(
            Annuity::Present,
            rate,
            periods - per + 1.0,
            payment,
            terminal,
            0.0,
        )? * rate
    } else if timing == 0.0 {
        annuity::calculate(Annuity::Future, rate, per - 1.0, payment, present, 0.0)? * rate
    } else {
        // At the second beginning-of-period payment, interest has accrued on
        // present+payment once. Later periods use the ordinary end-payment ledger.
        annuity::calculate(
            Annuity::Future,
            rate,
            per - 2.0,
            payment,
            present + payment,
            0.0,
        )? * rate
    };
    if !interest.is_finite() {
        return Err(EvalError::Overflow);
    }
    match function {
        FinancialFunction::IPmt => Ok(interest),
        FinancialFunction::PPmt => Ok(payment - interest),
        _ => Err(EvalError::UnsupportedExpression),
    }
}

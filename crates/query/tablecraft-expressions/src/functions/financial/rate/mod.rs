//! Annuity equation composition over the shared bounded financial solver.
mod equation;

use super::solver;
use crate::EvalError;
use equation::Equation;

pub(super) fn calculate(
    periods: f64,
    payment: f64,
    present: f64,
    future: f64,
    timing: f64,
    guess: f64,
    check: &mut impl FnMut() -> Result<(), EvalError>,
) -> Result<f64, EvalError> {
    if guess <= -1.0 {
        return Err(EvalError::Domain);
    }

    let equation = Equation::new(periods, payment, present, future, timing == 1.0)?;
    solver::solve(&equation, guess, check)
}

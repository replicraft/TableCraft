//! Bounded, guess-directed solving with residual qualification and host checkpoints.
mod equation;
use crate::EvalError;
use equation::Equation;

const MAX_ITERATIONS: usize = 20;
const RATE_TOLERANCE: f64 = 1e-7;
const RESIDUAL_TOLERANCE: f64 = 64.0 * f64::EPSILON;
const MAX_LOG_STEP: f64 = 1.0;
const INITIAL_LOG_PROBE: f64 = 1e-5;

#[derive(Clone, Copy)]
struct Point {
    x: f64,
    value: f64,
}

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
    let minimum = (-1.0 + f64::EPSILON / 2.0).ln_1p();
    let maximum = f64::MAX.ln();
    let mut x = guess.ln_1p();
    let mut previous: Option<Point> = None;
    let mut previous_rate = guess;
    let mut negative: Option<Point> = None;
    let mut positive: Option<Point> = None;
    for iteration in 0..MAX_ITERATIONS {
        check()?;
        let rate = x.exp_m1();
        if !rate.is_finite() || rate <= -1.0 {
            return Err(EvalError::Convergence);
        }
        // Evaluate the representable returned rate, not an unreturnable log point.
        x = rate.ln_1p();
        let sample = equation.sample(x)?;
        if sample.value.abs() <= RESIDUAL_TOLERANCE * sample.magnitude
            && (iteration == 0 || (rate - previous_rate).abs() <= RATE_TOLERANCE)
        {
            check()?;
            return Ok(rate);
        }
        let current = Point {
            x,
            value: sample.value,
        };
        if sample.value < 0.0 {
            negative = Some(current);
        } else if sample.value > 0.0 {
            positive = Some(current);
        }
        let mut next = if sample.derivative.is_finite() && sample.derivative != 0.0 {
            x - (sample.value / sample.derivative).clamp(-MAX_LOG_STEP, MAX_LOG_STEP)
        } else if let Some(old) = previous.filter(|p| p.value != sample.value) {
            x - (sample.value * (x - old.x) / (sample.value - old.value))
                .clamp(-MAX_LOG_STEP, MAX_LOG_STEP)
        } else {
            x + INITIAL_LOG_PROBE
        };
        if let (Some(a), Some(b)) = (negative, positive) {
            let low = a.x.min(b.x);
            let high = a.x.max(b.x);
            if !next.is_finite() || next <= low || next >= high {
                next = low + (high - low) / 2.0;
            }
        }
        if !next.is_finite() {
            return Err(EvalError::Convergence);
        }
        next = next.clamp(minimum, maximum);
        previous = Some(current);
        previous_rate = rate;
        x = next;
    }
    check()?;
    Err(EvalError::Convergence)
}

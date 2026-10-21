//! Cashflow equation and derivative in log-growth coordinates.
use crate::EvalError;

pub(super) struct Equation {
    periods: f64,
    payment: f64,
    present: f64,
    future: f64,
    begin: bool,
}
pub(super) struct Sample {
    pub value: f64,
    pub derivative: f64,
    pub magnitude: f64,
}
impl Equation {
    pub fn new(
        periods: f64,
        payment: f64,
        present: f64,
        future: f64,
        begin: bool,
    ) -> Result<Self, EvalError> {
        if periods <= 0.0 {
            return Err(EvalError::Domain);
        }
        let cash = [payment, present, future];
        if !cash.iter().any(|v| *v > 0.0) || !cash.iter().any(|v| *v < 0.0) {
            return Err(EvalError::Domain);
        }
        let scale = payment.abs().max(present.abs()).max(future.abs());
        let normalized = cash.map(|v| v / scale);
        if cash
            .iter()
            .zip(normalized)
            .any(|(before, after)| *before != 0.0 && after == 0.0)
        {
            return Err(EvalError::PrecisionLoss);
        }
        let [payment, present, future] = normalized;
        if periods == 1.0
            && ((!begin && present == 0.0 && payment + future == 0.0)
                || (begin && present + payment == 0.0 && future == 0.0))
        {
            return Err(EvalError::Domain);
        }
        Ok(Self {
            periods,
            payment,
            present,
            future,
            begin,
        })
    }
    pub fn sample(&self, x: f64) -> Result<Sample, EvalError> {
        let n = self.periods;
        let q = n * x;
        let (weights, slope) = if x == 0.0 {
            let log_slope = (n - 1.0) / 2.0 + if self.begin { 1.0 } else { 0.0 };
            ([1.0, n, 1.0], log_slope)
        } else {
            let z = x.abs();
            let total = q.abs();
            let ratio = if total.max(z) <= f64::EPSILON.sqrt() {
                n * relative(-total) / relative(-z)
            } else {
                -(-total).exp_m1() / (-(-z).exp_m1())
            };
            let mut slope = if total.max(z) <= f64::EPSILON.sqrt() {
                -(n - 1.0) / 2.0 + (n * total - z) / 12.0
            } else {
                n / total.exp_m1() - 1.0 / z.exp_m1()
            };
            if x > 0.0 {
                let end = if self.begin { 0.0 } else { 1.0 };
                slope -= end;
                ([1.0, ratio * (-end * z).exp(), (-total).exp()], slope)
            } else {
                let begin = if self.begin { 1.0 } else { 0.0 };
                slope = -slope + begin;
                ([(-total).exp(), ratio * (begin * x).exp(), 1.0], slope)
            }
        };
        let [a, b, c] = [
            self.present * weights[0],
            self.payment * weights[1],
            self.future * weights[2],
        ];
        let scale = a.abs().max(b.abs()).max(c.abs());
        if scale == 0.0 {
            return Err(EvalError::PrecisionLoss);
        }
        if !scale.is_finite() {
            return Err(EvalError::Overflow);
        }
        let [a, b, c] = [a / scale, b / scale, c / scale];
        let derivative = if x > 0.0 {
            b * slope - n * c
        } else {
            n * a + b * slope
        };
        Ok(Sample {
            value: (a + b) + c,
            derivative,
            magnitude: a.abs() + b.abs() + c.abs(),
        })
    }
}
fn relative(x: f64) -> f64 {
    if x == 0.0 { 1.0 } else { x.exp_m1() / x }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn analytic_slope_matches_an_independent_integer_period_cashflow_polynomial() {
        for begin in [false, true] {
            for x in [-0.5_f64, -0.01, -1e-12, 0., 1e-12, 0.01, 0.5] {
                let n = 10;
                let payment = -0.05;
                let present = 1.;
                let future = 0.1;
                let mut raw = present * (f64::from(n) * x).exp() + future;
                let mut derivative = present * f64::from(n) * (f64::from(n) * x).exp();
                for i in 0..n {
                    let power = f64::from(i + i32::from(begin));
                    let term = payment * (power * x).exp();
                    raw += term;
                    derivative += power * term;
                }
                let expected = derivative / raw - if x > 0. { f64::from(n) } else { 0. };
                let actual = Equation::new(f64::from(n), payment, present, future, begin)
                    .unwrap()
                    .sample(x)
                    .unwrap();
                assert!(
                    (actual.derivative / actual.value - expected).abs()
                        < expected.abs().max(1.) * 2e-12,
                    "{begin} {x}"
                );
            }
        }
    }
}

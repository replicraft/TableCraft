use super::super::solver::{self, RootEquation, Sample};
use super::{CashFlows, EvalError, Work, rate, sum::Horner};

struct Equation<'a>(&'a CashFlows);

impl RootEquation for Equation<'_> {
    fn sample(
        &self,
        x: f64,
        check: &mut impl FnMut() -> Result<(), EvalError>,
    ) -> Result<Sample, EvalError> {
        let factor = (-x.abs()).exp();
        let mut sum = Horner::default();
        let mut derivative = 0.;
        let mut magnitude = 0.;
        let forward = x < 0.0;
        let length = self.0.values.len();

        for position in 0..length {
            check()?;
            let index = if forward {
                position
            } else {
                length - 1 - position
            };
            let value = self.0.normalized(self.0.values[index])?;

            let old = sum.value()?;
            derivative = factor * (derivative + if forward { old } else { -old });
            sum.push(factor, value)?;
            magnitude = magnitude * factor + value.abs();
        }

        if magnitude == 0.0 {
            return Err(EvalError::PrecisionLoss);
        }

        Ok(Sample {
            value: sum.value()?,
            derivative,
            magnitude,
        })
    }
}

pub(super) fn calculate(
    flows: &CashFlows,
    guess: f64,
    work: &mut Work<'_>,
) -> Result<f64, EvalError> {
    rate(guess)?;
    flows.mixed()?;

    solver::solve(&Equation(flows), guess, &mut || work.check())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tablecraft_core::{
        limits::ResourceBudget,
        values::{Double, Value},
    };

    #[test]
    fn scaled_samples_and_derivatives_match_independent_period_polynomials() {
        let numbers = [-100.0_f64, 12.0, 34.0, 0.0, 80.0];
        let values: Vec<Value> = numbers
            .iter()
            .map(|&value| Value::Double(Double::from_float(value)))
            .collect();
        let mut budget = ResourceBudget::new(1024, 0, 1000);
        let flows = CashFlows::new(&values, &mut budget, None).unwrap();

        for x in [-1.0_f64, -0.1, -1e-10, 0.0, 1e-10, 0.1, 1.0] {
            let mut expected_value = 0.0;
            let mut expected_derivative = 0.0;
            let mut expected_magnitude = 0.0;

            for (index, value) in numbers.iter().enumerate() {
                let power = if x < 0.0 {
                    (numbers.len() - 1 - index) as f64
                } else {
                    -(index as f64)
                };
                let term = value / 100.0 * (power * x).exp();
                expected_value += term;
                expected_derivative += term * power;
                expected_magnitude += term.abs();
            }

            let sample = Equation(&flows).sample(x, &mut || Ok(())).unwrap();
            for (actual, expected) in [
                (sample.value, expected_value),
                (sample.derivative, expected_derivative),
                (sample.magnitude, expected_magnitude),
            ] {
                assert!(
                    (actual - expected).abs() < 1e-12,
                    "{x}: {actual} != {expected}"
                );
            }
        }
    }
}

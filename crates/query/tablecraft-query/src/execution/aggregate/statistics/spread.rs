//! Shifted, scaled centered deviations with compensated accumulation.
use crate::{QueryError, plan::Statistic};
use tablecraft_expressions::EvalError;

#[derive(Default)]
struct Sum {
    value: f64,
    correction: f64,
}
impl Sum {
    fn add(&mut self, value: f64) {
        let next = self.value + value;
        self.correction += if self.value.abs() >= value.abs() {
            (self.value - next) + value
        } else {
            (value - next) + self.value
        };
        self.value = next;
    }
    fn total(self) -> f64 {
        self.value + self.correction
    }
}
fn shifted(value: f64, anchor: f64, scale: f64) -> f64 {
    let delta = value - anchor;
    if delta.is_finite() {
        delta / scale
    } else {
        value / scale - anchor / scale
    }
}
pub(super) fn calculate(
    values: &[f64],
    scale: f64,
    denominator: u32,
    kind: Statistic,
    step: &mut dyn FnMut() -> Result<(), QueryError>,
) -> Result<f64, QueryError> {
    if scale == 0.0 {
        return Ok(0.0);
    }
    let anchor = *values.first().ok_or(QueryError::Grouping)?;
    let mut sum = Sum::default();
    for &value in values {
        step()?;
        sum.add(shifted(value, anchor, scale));
    }
    let count = u32::try_from(values.len()).map_err(|_| QueryError::StructureLimit)?;
    let mean = sum.total() / f64::from(count);
    let mut squares = Sum::default();
    for &value in values {
        step()?;
        let delta = shifted(value, anchor, scale) - mean;
        squares.add(delta * delta);
    }
    let normalized = squares.total() / f64::from(denominator);
    // Avoid scale*scale overflow when the final variance is representable.
    // Deviation never forms a potentially unrepresentable variance first.
    let result = if kind.deviation() {
        normalized.sqrt() * scale
    } else {
        (normalized * scale) * scale
    };
    if result.is_finite() {
        Ok(result)
    } else {
        Err(EvalError::Overflow.into())
    }
}

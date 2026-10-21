//! Compensated Horner updates, keeping multiplication and addition residuals.
use super::{EvalError, finite};

#[derive(Default)]
pub(super) struct Horner {
    value: f64,
    correction: f64,
}

impl Horner {
    pub fn push(&mut self, factor: f64, coefficient: f64) -> Result<(), EvalError> {
        let product = finite(self.value * factor)?;
        let error = self.value.mul_add(factor, -product);
        let sum = finite(product + coefficient)?;
        let addition = if product.abs() >= coefficient.abs() {
            (product - sum) + coefficient
        } else {
            (coefficient - sum) + product
        };

        self.correction = finite(self.correction * factor + error + addition)?;
        self.value = sum;
        Ok(())
    }

    pub fn value(&self) -> Result<f64, EvalError> {
        finite(self.value + self.correction)
    }
}

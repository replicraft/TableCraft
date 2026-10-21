//! Shared metering and finite numeric input checks.
use crate::{EvalError, functions::conversions::Conversion};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

pub(super) struct Work<'a> {
    pub(super) budget: &'a mut ResourceBudget,
    pub(super) cancelled: Option<&'a dyn Fn() -> bool>,
}

impl Work<'_> {
    pub(super) fn check(&mut self) -> Result<(), EvalError> {
        if self.cancelled.is_some_and(|f| f()) {
            return Err(EvalError::Cancelled);
        }

        self.budget.charge(Resource::Steps, 1)?;
        Ok(())
    }
}

pub(super) fn number(value: &Value) -> Result<f64, EvalError> {
    let Value::Double(value) = Conversion::Double.apply(value)? else {
        return Err(EvalError::TypeMismatch);
    };

    Ok(value.to_float())
}

pub(super) fn finite(value: f64) -> Result<f64, EvalError> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(EvalError::Overflow)
    }
}

pub(super) fn rate(value: f64) -> Result<(), EvalError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(EvalError::NonFinite)
    }
}
